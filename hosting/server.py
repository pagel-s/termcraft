#!/usr/bin/env python3
"""Start a private, rootless Podman server. Requires Linux and Python 3.9+."""
import argparse
import hashlib
import ipaddress
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
MANAGED = 'io.termcraft.server'
MARKER = '.termcraft-server-data'
PRIVATE = tuple(ipaddress.ip_network(n) for n in (
    '127.0.0.0/8', '10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16', '100.64.0.0/10'))


def run(*args, capture=True):
    return subprocess.run(['podman', '--remote=false', *map(str, args)], check=True,
                          text=True, stdout=subprocess.PIPE if capture else None)


def rootless():
    if sys.platform != 'linux' or os.geteuid() == 0:
        raise ValueError('Use Linux as a regular user, without sudo. See docs/hosting.md.')
    if not shutil.which('podman'):
        raise ValueError('Install Podman first; see docs/hosting.md. No rootful fallback is used.')
    info = json.loads(run('info', '--format', 'json').stdout)
    if not info.get('host', {}).get('security', {}).get('rootless'):
        raise ValueError('Podman must report rootless=true; see docs/hosting.md.')
    host = info['host']
    if host.get('cgroupVersion') != 'v2' or not {'cpu', 'memory', 'pids'} <= set(host.get('cgroupControllers', [])):
        raise ValueError('Rootless hosting needs cgroup v2 with delegated cpu, memory and pids controllers. See docs/hosting.md; resource limits will not be silently disabled.')


def data_directory(path):
    if sys.platform != 'linux' or os.geteuid() == 0:
        raise ValueError('Use Linux as a regular user, without sudo.')
    if path.is_symlink():
        raise ValueError('The save directory must not be a symbolic link.')
    path = path.expanduser().resolve()
    if path in (Path('/'), Path.home().resolve(), ROOT) or any(c in str(path) for c in ':,\n\r'):
        raise ValueError('Choose a dedicated save directory, separate from home and the game folder.')
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.stat().st_uid != os.geteuid():
        raise ValueError('The save directory must belong to your current user.')
    marker = path / MARKER
    if not marker.exists():
        if any(path.iterdir()):
            raise ValueError('Refusing to mount a nonempty, unmarked directory. Use init before importing a world.')
        marker.write_text('termcraft-server-data-v1\n')
        marker.chmod(0o600)
    if marker.is_symlink() or marker.read_text() != 'termcraft-server-data-v1\n':
        raise ValueError('Invalid save-directory marker; no container was started.')
    path.chmod(0o700)
    return path


def inspect(name):
    result = subprocess.run(['podman', '--remote=false', 'container', 'exists', name])
    if result.returncode == 1:
        return None
    result.check_returncode()
    item = json.loads(run('inspect', '--type', 'container', name).stdout)[0]
    if item.get('Config', {}).get('Labels', {}).get(MANAGED) != '1':
        raise ValueError(f'Refusing to manage unrelated container {name}. Choose a different --name.')
    return item


def image_for(binary):
    binary = binary.expanduser().resolve()
    if not binary.is_file() or binary.read_bytes()[:4] != b'\x7fELF':
        raise ValueError('Use a Linux Termcraft executable; pass --binary PATH if needed.')
    version = subprocess.check_output([str(binary), '--version'], text=True).strip()
    if not re.fullmatch(r'termcraft \d+\.\d+\.\d+(?:[-+][0-9A-Za-z.+-]+)?', version):
        raise ValueError('The executable did not identify itself as Termcraft.')
    recipe = Path(__file__).with_name('Containerfile')
    digest = hashlib.sha256(binary.read_bytes() + recipe.read_bytes() + (ROOT / 'LICENSE').read_bytes()).hexdigest()
    image = 'localhost/termcraft-server:' + digest[:20]
    exists = subprocess.run(['podman', '--remote=false', 'image', 'exists', image])
    if exists.returncode not in (0, 1):
        exists.check_returncode()
    if exists.returncode == 1:
        # Send only these three files to the image builder, never the checkout.
        with tempfile.TemporaryDirectory(prefix='termcraft-image-') as directory:
            context = Path(directory)
            shutil.copy2(binary, context / 'termcraft')
            (context / 'termcraft').chmod(0o755)
            shutil.copy2(recipe, context / 'Containerfile')
            shutil.copy2(ROOT / 'LICENSE', context / 'LICENSE')
            run('build', '--network=none', '--label', f'io.termcraft.build={digest}',
                '-t', image, '-f', context / 'Containerfile', context, capture=False)
    return image


def start(args):
    address = ipaddress.IPv4Address(args.bind)
    if not any(address in network for network in PRIVATE):
        raise ValueError('--bind must be loopback, a private LAN IPv4 address, or a Tailscale IPv4 address. Wildcard/public bindings are refused.')
    if not 1 <= args.port <= 65535 or not math.isfinite(args.cpus) or args.cpus <= 0:
        raise ValueError('Choose a port from 1 to 65535 and a positive CPU limit.')
    if not re.fullmatch(r'[1-9][0-9]*[kKmMgG]?', args.memory):
        raise ValueError('Choose a positive memory limit such as 512m or 1g; unlimited memory is refused.')
    data = data_directory(args.data)
    old = inspect(args.name)
    if old:
        if old['State']['Running']:
            raise ValueError('The server is already running. Stop it before changing its binary, address or saves.')
        if old['State']['ExitCode'] != 0:
            raise ValueError('The previous server exited abnormally. Read logs and the recovery section in docs/hosting.md before removing its container.')
        run('rm', args.name)
    image = image_for(args.binary)
    run('run', '--detach', '--name', args.name, '--label', MANAGED + '=1',
        '--userns=keep-id', '--user', f'{os.getuid()}:{os.getgid()}',
        '--read-only', '--cap-drop=ALL', '--security-opt=no-new-privileges',
        '--memory', args.memory, '--cpus', args.cpus, '--pids-limit', '128',
        '--tmpfs', '/tmp:rw,noexec,nosuid,nodev,size=64m,mode=1777',
        '--volume', f'{data}:/data:rw,Z',
        '--publish', f'{address}:{args.port}:25565/tcp', image)
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        state = inspect(args.name)['State']
        logs = subprocess.run(['podman', '--remote=false', 'logs', args.name],
                              check=True, capture_output=True, text=True)
        log = logs.stdout + logs.stderr
        if not state['Running']:
            raise ValueError('Server startup failed. Inspect logs with: python3 hosting/server.py logs --name ' + args.name)
        if 'Starting headless server on ' in log:
            print(f'Server ready. Join: ./termcraft client {address}:{args.port} --player alice')
            print(f'Saves: {data}\nStop: python3 hosting/server.py stop --name {args.name}')
            return
        time.sleep(0.25)
    raise ValueError('Startup is taking longer than 60 seconds; the container is retained. Inspect its status/logs before stopping or restarting it.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('init', 'build', 'start', 'stop', 'status', 'logs'))
    parser.add_argument('--name', default='termcraft-server')
    parser.add_argument('--data', type=Path, default=Path.home() / '.local/share/termcraft-server')
    parser.add_argument('--binary', type=Path, default=ROOT / 'termcraft')
    parser.add_argument('--bind', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=25565)
    parser.add_argument('--memory', default='1g')
    parser.add_argument('--cpus', type=float, default=2)
    args = parser.parse_args()
    if not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_-]{0,62}', args.name):
        parser.error('choose a container name of 1–63 letters, digits, underscores or hyphens')
    try:
        if args.action == 'init':
            print(data_directory(args.data))
            return
        rootless()
        if args.action == 'build':
            print(image_for(args.binary))
        elif args.action == 'start':
            start(args)
        else:
            item = inspect(args.name)
            if not item:
                if args.action == 'stop':
                    raise ValueError('No server container exists with this name. Check --name before assuming your world is stopped.')
                print('No server container exists with this name.')
            elif args.action == 'logs':
                run('logs', args.name, capture=False)
            elif args.action == 'status':
                print(json.dumps(dict(name=args.name, running=item['State']['Running'],
                                      exit_code=item['State']['ExitCode'],
                                      ports=item['NetworkSettings']['Ports'],
                                      saves=[m['Source'] for m in item['Mounts'] if m['Destination'] == '/data']), indent=2))
            else:
                run('stop', '--time', '60', args.name)
                if inspect(args.name)['State']['ExitCode'] != 0:
                    raise ValueError('The server did not stop cleanly. Keep the saves intact and inspect logs before restarting.')
                print('Server stopped and saved. Its data folder and logs are retained.')
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'Error: {error}', file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
