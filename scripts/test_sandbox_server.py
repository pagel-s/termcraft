#!/usr/bin/env python3
"""Qualify the packaged rootless server, real terminal players and backup restore."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from test_terminal_input import TerminalGame


def podman(*args):
    return subprocess.check_output(['podman', '--remote=false', *map(str, args)], text=True)


def check(bundle, root):
    binary = bundle / 'termcraft'
    launcher = bundle / 'hosting/server.py'
    name = 'termcraft-qualification-' + str(os.getpid())
    data = root / 'server data'
    clients = root / 'client saves'
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    address = '127.0.0.1:' + str(port)
    players = []

    def command(action, directory=data, executable=binary):
        result = subprocess.run([sys.executable, str(launcher), action, '--name', name,
                                 '--data', str(directory), '--binary', str(executable),
                                 '--port', str(port)], capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stdout + result.stderr
        return result.stdout

    def state():
        return json.loads(podman('inspect', name))[0]

    def join(profile):
        game = TerminalGame(binary, clients, root / profile, address=address,
                            observe=False, start_server=False, profile=profile)
        players.append(game)
        return game

    def leave_players():
        for game in players:
            if game.process.poll() is None:
                game.finish()
            game.close()
        players.clear()

    try:
        info = json.loads(podman('info', '--format', 'json'))
        assert info['host']['security']['rootless'] and os.geteuid() != 0
        command('start')
        configuration = state()
        host = configuration['HostConfig']
        assert host['ReadonlyRootfs']
        assert host['Memory'] == 1024 ** 3
        assert host['PidsLimit'] == 128
        assert host.get('NanoCpus') == 2_000_000_000 or (host.get('CpuPeriod', 0) > 0 and host['CpuQuota'] / host['CpuPeriod'] == 2)
        assert any('no-new-privileges' in option for option in host['SecurityOpt'])
        assert configuration['Config']['User'] == f'{os.getuid()}:{os.getgid()}'
        assert configuration['NetworkSettings']['Ports']['25565/tcp'] == [dict(HostIp='127.0.0.1', HostPort=str(port))]
        assert [(m['Source'], m['Destination']) for m in configuration['Mounts'] if m['Type'] == 'bind'] == [(str(data), '/data')]
        assert podman('exec', name, 'id', '-u').strip() == str(os.getuid())
        failed_write = subprocess.run(['podman', '--remote=false', 'exec', name, 'touch', '/etc/termcraft-test'], capture_output=True)
        assert failed_write.returncode != 0, 'container root filesystem is writable'
        status = podman('exec', name, 'cat', '/proc/1/status')
        assert 'NoNewPrivs:\t1' in status
        assert 'CapEff:\t0000000000000000' in status
        assert podman('exec', name, 'cat', '/sys/fs/cgroup/memory.max').strip() == str(1024 ** 3)
        assert podman('exec', name, 'cat', '/sys/fs/cgroup/pids.max').strip() == '128'
        quota, period = podman('exec', name, 'cat', '/sys/fs/cgroup/cpu.max').split()
        assert int(quota) == int(period) * 2
        assert not (data.stat().st_mode & 0o077), 'save directory is not private'

        alice, bob, charlie = [join(p) for p in ('alice', 'bob', 'charlie')]
        charlie.send(b'tsandbox-three-player-check\r'); charlie.pump(0.2)
        alice.pump(0.2); bob.pump(0.2)
        assert 'sandbox-three-player-check' in alice.screen.text()
        assert 'sandbox-three-player-check' in bob.screen.text()
        alice.send(b't/gamemode on\r'); alice.pump(0.2)
        alice.send(b'\x1b[20~'); alice.pump(0.3)
        alice.capture('sandbox-equipped')
        leave_players()
        assert state()['State']['Running'], 'dedicated server stopped when a player left'
        command('stop')
        assert state()['State']['ExitCode'] == 0
        assert not (data / 'multiplayer/default/.session.lock').exists()
        saved = json.loads((data / 'multiplayer/default/server-state.json').read_text())
        assert len(saved['players']) == 3
        inventories = {token: player['inventory'] for token, player in saved['players'].items()}
        assert any(any(slot is not None for slot in inventory['slots']) for inventory in inventories.values())

        backup = root / 'world-backup.tar.gz'
        subprocess.run(['tar', '-czf', str(backup), '-C', str(data), '.'], check=True)
        restored = root / 'restored data'
        restored.mkdir(mode=0o700)
        subprocess.run(['tar', '-xzf', str(backup), '-C', str(restored)], check=True)
        before = {p.relative_to(data): hashlib.sha256(p.read_bytes()).hexdigest() for p in data.rglob('*') if p.is_file()}
        after = {p.relative_to(restored): hashlib.sha256(p.read_bytes()).hexdigest() for p in restored.rglob('*') if p.is_file()}
        assert before == after, 'backup omitted files or hidden checkpoint state'

        # A different valid executable must select a new image while retaining
        # world/player records. ELF permits trailing bytes; no game code changes.
        replacement = root / 'updated-termcraft'
        shutil.copy2(binary, replacement)
        with replacement.open('ab') as output:
            output.write(b'\x00')
        command('start', restored, replacement)
        assert state()['Image'] != configuration['Image'], 'updated binary reused the old image'
        resumed = join('alice')
        resumed.pump(0.2)
        leave_players()
        command('stop')
        reopened = json.loads((restored / 'multiplayer/default/server-state.json').read_text())
        assert reopened['id'] == saved['id']
        assert {token: p['inventory'] for token, p in reopened['players'].items()} == inventories
        assert not (restored / 'multiplayer/default/.session.lock').exists()
        return dict(status='passed', rootless=True, nonroot_process=True,
                    read_only_root=True, capabilities_dropped=True, no_new_privileges=True,
                    private_save_mount_only=True, loopback_publish_only=True,
                    resource_limits=True, three_terminal_players=True,
                    dedicated_server_outlives_players=True, graceful_shutdown=True,
                    backup_restores_all_files=True, updated_image_preserves_players=True,
                    podman_version=info['version']['Version'],
                    limits='Linux rootless Podman, loopback TCP and native PTYs. No live Tailscale policy, WAN, public exposure or container-escape security audit.')
    finally:
        if sys.exc_info()[0] is not None:
            logs = subprocess.run(['podman', '--remote=false', 'logs', name], capture_output=True, text=True)
            print(logs.stdout + logs.stderr, file=sys.stderr)
        for player in players:
            player.close()
        subprocess.run(['podman', '--remote=false', 'rm', '--force', name], capture_output=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='termcraft-sandbox-') as directory:
        result = check(args.bundle.resolve(), Path(directory))
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
