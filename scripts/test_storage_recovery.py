#!/usr/bin/env python3
"""Linux checkpoint fault checks through real CLI/MCP in disposable trial saves.

An explicitly scoped LD_PRELOAD test library injects ENOSPC, partial writes,
fsync EIO, slow writes and process exit at the rename boundary. No real disk is
filled or mounted, and no existing world is opened. Requires a C compiler.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import select
import shutil
import subprocess
import sys
import tempfile
import time


class Mcp:
    def __init__(self, binary, saves, fault_env=None):
        env = dict(os.environ, TERMCRAFT_SAVE_DIR=str(saves))
        if fault_env:
            env.update(fault_env)
        self.process = subprocess.Popen([str(binary), 'mcp'], env=env, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.sequence = 0

    def send(self, name, arguments=None):
        self.sequence += 1
        request = dict(jsonrpc='2.0', id=self.sequence, method='tools/call',
                       params=dict(name=name, arguments=arguments or {}))
        self.process.stdin.write((json.dumps(request) + '\n').encode())
        self.process.stdin.flush()
        return self.sequence

    def receive(self, timeout=10):
        readable, _, _ = select.select([self.process.stdout], [], [], timeout)
        assert readable, 'MCP response timed out'
        line = self.process.stdout.readline()
        if not line:
            return None
        return json.loads(line)['result']

    def call(self, name, arguments=None):
        self.send(name, arguments)
        result = self.receive()
        assert result and not result.get('isError'), result
        return result['structuredContent']

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdin.close()
        self.process.stdout.close()
        self.process.stderr.close()


def count(state, item):
    return sum(s['count'] for s in state['inventory'] if s and s['item_type'] == item)


def own_stale_lock(world, process):
    assert process.poll() is not None, 'never remove a live writer lease'
    lock = world / '.session.lock'
    if lock.exists():
        assert lock.read_text().strip() == str(process.pid)
        lock.unlink()


def check_case(binary, library, template, root, fault):
    saves = root / fault
    shutil.copytree(template, saves)
    world = saves / 'worlds' / 'world-audit'
    checkpoint = world / 'player_progression.bin'
    control, marker = saves / 'fault-mode', saves / 'fault-hits'
    control.write_text('off')
    fault_env = dict(LD_PRELOAD=str(library), TERMCRAFT_TEST_FAULT_ROOT=str(world),
                     TERMCRAFT_TEST_FAULT_CONTROL=str(control), TERMCRAFT_TEST_FAULT_MARKER=str(marker))
    client = Mcp(binary, saves, fault_env)
    try:
        before = client.call('enter_world', dict(id='audit'))
        recipes = client.call('recipes')['recipes']
        recipe = next(r for r in recipes if r['result'] == 'Stick' and r['craftable'])
        previous = checkpoint.read_bytes()
        control.write_text(fault)
        client.send('craft', dict(recipe_id=recipe['id']))
        if fault == 'slow':
            deadline = time.monotonic() + 5
            while not marker.exists():
                assert time.monotonic() < deadline, 'fault interposer did not activate'
                time.sleep(0.005)
            assert 'slow-write-enter' in marker.read_text()
            assert checkpoint.read_bytes() == previous, 'partial slow checkpoint was published'
        result = client.receive()
        trace = marker.read_text() if marker.exists() else ''
        assert trace, 'fault interposer did not hit the actual checkpoint path'
        if fault.startswith('crash-'):
            assert result is None, result
            assert client.process.wait(timeout=5) == (97 if fault == 'crash-before' else 98)
            own_stale_lock(world, client.process)
            changed = fault == 'crash-after'
            assert (checkpoint.read_bytes() != previous) == changed
        else:
            assert result is not None
            changed = True
            if fault != 'slow':
                assert result.get('isError'), result
                assert checkpoint.read_bytes() == previous, 'failed write replaced the checkpoint'
                if fault == 'partial':
                    assert 'partial-write' in trace and 'partial-enospc' in trace
                    pending = world / 'player_progression.bin.tmp'
                    assert pending.exists() and 0 < pending.stat().st_size < len(previous)
            else:
                assert not result.get('isError'), result
            in_memory = client.call('observe')
            assert count(in_memory, 'Stick') == count(before, 'Stick') + 4
            assert count(in_memory, 'Planks') == count(before, 'Planks') - 2
            control.write_text('off')
            client.call('save_world')
            client.call('leave_world')
    finally:
        client.close()
    # Reopening goes through normal world ownership and validation with no hooks.
    restored = Mcp(binary, saves)
    try:
        actual = restored.call('enter_world', dict(id='audit'))
        assert count(actual, 'Stick') == count(before, 'Stick') + (4 if changed else 0)
        assert count(actual, 'Planks') == count(before, 'Planks') - (2 if changed else 0)
        restored.call('leave_world')
    finally:
        restored.close()
    return dict(fault=fault, status='passed', hits=trace.splitlines(),
                previous_checkpoint_preserved_until_commit=fault != 'crash-after',
                normal_rule_inventory_restored=True, retry_after_error=not fault.startswith('crash-'))


def native_recovery(binary, library, template, root):
    # Native PTY helper needs Pillow; no image captures are required for this check.
    from test_terminal_input import TerminalGame
    import termios
    saves = root / 'native-recovery'
    shutil.copytree(template, saves)
    world = saves / 'worlds' / 'world-audit'
    control, marker = saves / 'fault-mode', saves / 'fault-hits'
    control.write_text('off')
    original = (world / 'player_progression.bin').read_bytes()
    backup = root / 'backup'
    shutil.copytree(template, backup)
    existing_recoveries = set(Path(tempfile.gettempdir()).glob('termcraft-recovery-*'))
    game = TerminalGame(binary, saves, root / 'pty', 'audit', env_overrides=dict(
        LD_PRELOAD=str(library), TERMCRAFT_TEST_FAULT_ROOT=str(world),
        TERMCRAFT_TEST_FAULT_CONTROL=str(control), TERMCRAFT_TEST_FAULT_MARKER=str(marker)))
    recovery = None
    try:
        start_x = game.x()
        game.send(b'd')
        game.pump(0.6)  # Allow the bounded plain-key hold to release.
        moved_x = game.x()
        game.pump(0.15)
        assert abs(game.x() - moved_x) < 0.01, 'fixture did not stop'
        assert moved_x > start_x + 0.2, 'native input did not move the player'
        control.write_text('full')
        game.send(b'q')
        deadline = time.monotonic() + 8
        while game.process.poll() is None:
            assert time.monotonic() < deadline, 'save-error shutdown stalled'
            game.pump(0.05)
        game.pump(0.1)
        assert game.process.returncode != 0, 'injected save failure was ignored'
        assert 'write-enospc' in marker.read_text()
        assert b'\x1b[?1049l' in game.raw, 'alternate screen not restored after failure'
        assert game.before == termios.tcgetattr(game.slave), 'terminal modes not restored after failure'
        text = game.raw.decode('utf-8', errors='replace')
        match = re.search(r'saved to (.*?termcraft-recovery-[0-9a-f]{32})\.', text)
        assert match, text[-2500:]
        recovery = Path(match.group(1))
        assert recovery not in existing_recoveries and recovery.parent == Path(tempfile.gettempdir())
        assert recovery.stat().st_uid == os.getuid()
        assert (recovery.stat().st_mode & 0o777) == 0o700
        assert (recovery / 'RECOVERY.txt').is_file()
        assert (world / 'player_progression.bin').read_bytes() == original
        assert (backup / 'worlds/world-audit/player_progression.bin').read_bytes() == original
        # Follow the documented backup -> second copy -> overlay -> reopen path.
        working = root / 'recovered-copy'
        shutil.copytree(backup, working)
        destination = working / 'worlds' / 'world-audit'
        allowed = {'player_progression.bin', 'world.json', 'environment.json', 'director.json',
                   'lantern-run.json', 'RECOVERY.txt'}
        copied = []
        for file in recovery.iterdir():
            if file.is_file() and not file.is_symlink() and (
                file.name in allowed or re.fullmatch(r'(overworld|nether|end)_(chunk_-?\d+|world_seed)\.bin', file.name)
            ):
                shutil.copyfile(file, destination / file.name)
                copied.append(file.name)
        restored = Mcp(binary, working)
        try:
            observed = restored.call('enter_world', dict(id='audit'))
            assert abs(observed['player']['x'] - moved_x) < 0.01
            restored.call('leave_world')
            observed = restored.call('enter_world', dict(id='audit'))
            assert abs(observed['player']['x'] - moved_x) < 0.01
            restored.call('leave_world')
        finally:
            restored.close()
        return dict(status='passed', terminal_restored_after_save_error=True,
                    original_checkpoint_and_backup_preserved=True, copied_world_reopened_twice=True,
                    private_recovery_directory=True, start_x=start_x, restored_x=moved_x,
                    overlay_files=sorted(copied))
    finally:
        game.close()
        if recovery and recovery not in existing_recoveries and recovery.is_dir():
            shutil.rmtree(recovery)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('termcraft'))
    parser.add_argument('--report', type=Path)
    parser.add_argument('--native-recovery', action='store_true', help='Also test native PTY failure and backup-copy recovery; needs Pillow')
    args = parser.parse_args()
    if sys.platform != 'linux':
        parser.error('fault interposition uses Linux /proc and LD_PRELOAD')
    compiler = shutil.which('cc')
    if not compiler:
        parser.error('C compiler cc is required')
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix='termcraft-storage-') as directory:
        root = Path(directory)
        library = root / 'storage-faults.so'
        source = Path(__file__).with_name('storage_faults.c')
        subprocess.run([compiler, '-std=c11', '-shared', '-fPIC', '-O2', '-Wall', '-Wextra',
                        '-Werror', '-pthread', str(source), '-ldl', '-o', str(library)], check=True)
        template = root / 'template'
        subprocess.run([str(binary), 'trial', 'create', 'audit'],
                       env=dict(os.environ, TERMCRAFT_SAVE_DIR=str(template)),
                       capture_output=True, check=True)
        cases = [check_case(binary, library, template, root, mode)
                 for mode in ['full', 'partial', 'sync', 'slow', 'crash-before', 'crash-after']]
        report = dict(status='passed', binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                      cases=cases, disposable_worlds_only=True,
                      limits='Process-scoped faults on native checkpoint writes; real CLI crafting/save/reopen. '
                             'Not physical-disk power-loss durability or a whole-world transaction test.')
        if args.native_recovery:
            report['native_recovery'] = native_recovery(binary, library, template, root)
        if args.report:
            args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report))


if __name__ == '__main__':
    main()
