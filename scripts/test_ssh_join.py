#!/usr/bin/env python3
"""Unix SSH integration test: real OpenSSH client, disposable Paramiko endpoint.

Requires Python 3.11+, Paramiko and ssh in PATH. No host daemon/account/config is
changed. All keys, saves and listeners belong to a temporary loopback fixture.
This verifies SSH transport and the join UI, not remote-account administration.
"""
import argparse
import fcntl
import hashlib
import json
import logging
import os
from pathlib import Path
import pty
import select
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

import paramiko


def wait_for(predicate, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError("fixture condition timed out")


def process_exists(pid):
    try:
        os.kill(int(pid), 0)
        return True
    except ProcessLookupError:
        return False


def close_stream(stream):
    try:
        stream.close()
    except (OSError, EOFError, paramiko.SSHException):
        pass  # A deliberately dropped SSH transport cannot send channel-close.


class Endpoint:
    def __init__(self, host_key, client_key, game_port):
        self.host_key, self.client_key, self.game_port = host_key, client_key, game_port
        self.mode = "allow"
        self.transports = []
        self.authenticated = 0
        self.encrypted = 0
        self.forwarded = 0
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen()
        self.listener.settimeout(0.1)
        self.port = self.listener.getsockname()[1]
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.accept, daemon=True)
        self.thread.start()

    def accept(self):
        while not self.stop.is_set():
            try:
                stream, _ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            threading.Thread(target=self.session, args=(stream,), daemon=True).start()

    def session(self, stream):
        endpoint = self

        class Policy(paramiko.ServerInterface):
            def get_allowed_auths(self, username):
                return "publickey"

            def check_auth_publickey(self, username, key):
                if endpoint.mode != "deny-auth" and username == "fixture" and key == endpoint.client_key:
                    return paramiko.AUTH_SUCCESSFUL
                return paramiko.AUTH_FAILED

            def check_channel_direct_tcpip_request(self, chanid, origin, destination):
                if endpoint.mode == "deny-forward" or destination != ("127.0.0.1", endpoint.game_port):
                    return paramiko.OPEN_FAILED_ADMINISTRATIVELY_PROHIBITED
                return paramiko.OPEN_SUCCEEDED

        transport = paramiko.Transport(stream)
        self.transports.append(transport)
        try:
            transport.add_server_key(self.host_key)
            transport.start_server(server=Policy())
            recorded = False
            while transport.is_active() and not self.stop.is_set():
                if transport.is_authenticated() and not recorded:
                    self.authenticated += 1
                    assert transport.local_cipher != "none" and transport.remote_cipher != "none"
                    self.encrypted += 1
                    recorded = True
                channel = transport.accept(0.1)
                if channel is not None:
                    self.forwarded += 1
                    threading.Thread(target=self.forward, args=(channel,), daemon=True).start()
        except (OSError, EOFError, paramiko.SSHException):
            pass  # Deliberate key/auth/link failures are part of this fixture.
        finally:
            transport.close()

    def forward(self, channel):
        try:
            target = socket.create_connection(("127.0.0.1", self.game_port), timeout=2)
        except OSError:
            close_stream(channel)
            return
        target.settimeout(None)

        def pipe(source, sink):
            try:
                while data := source.recv(65536):
                    sink.sendall(data)
            except (OSError, EOFError):
                pass
            finally:
                close_stream(channel)
                close_stream(target)

        threading.Thread(target=pipe, args=(channel, target), daemon=True).start()
        pipe(target, channel)

    def close(self):
        self.stop.set()
        self.listener.close()
        for transport in self.transports:
            transport.close()
        self.thread.join(timeout=2)


class Client:
    def __init__(self, args, env):
        self.master, self.slave = pty.openpty()
        self.resize(120, 40)
        self.before = termios.tcgetattr(self.slave)
        self.output = bytearray()

        def terminal():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

        self.process = subprocess.Popen(args, env=env, stdin=self.slave, stdout=self.slave,
                                        stderr=self.slave, preexec_fn=terminal)

    def resize(self, width, height):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def pump(self):
        if select.select([self.master], [], [], 0.01)[0]:
            self.output.extend(os.read(self.master, 65536))
        return b"Termcraft |" in self.output

    def drain(self):
        while select.select([self.master], [], [], 0.01)[0]:
            self.pump()

    def quit(self):
        # Drain a resize/initial frame before sending input, as a real terminal
        # does continuously. Otherwise a full PTY can block the rendering thread.
        deadline = time.monotonic() + 0.2
        while time.monotonic() < deadline:
            self.pump()
        os.write(self.master, b"q")
        wait_for(lambda: (self.pump(), self.process.poll() is not None)[1])
        self.drain()
        assert self.process.returncode == 0
        assert b"\x1b[?1049l" in self.output
        assert self.before == termios.tcgetattr(self.slave), "terminal settings not restored"

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=3)
        os.close(self.master)
        os.close(self.slave)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/termcraft"))
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve())
    ssh = shutil.which("ssh")
    if not ssh:
        parser.error("OpenSSH client is required")
    logging.getLogger("paramiko").addHandler(logging.NullHandler())
    logging.getLogger("paramiko").propagate = False
    clients, server, endpoint = [], None, None
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="termcraft-real-ssh-") as temporary:
        root = Path(temporary)
        env = dict(os.environ, TERMCRAFT_SAVE_DIR=str(root / "saves"), TERM="xterm-256color")
        env.pop("TERMCRAFT_DIRECTOR", None)
        env.pop("NO_COLOR", None)
        recipe = json.loads(subprocess.check_output([binary, "world", "schema"]))["example"]
        recipe.update(terrain="plains", relief=0, regions=[], landmarks=[])
        (root / "recipe.json").write_text(json.dumps(recipe))
        subprocess.run([binary, "world", "create", "ssh", str(root / "recipe.json")], env=env,
                       check=True, capture_output=True)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            game_port = reservation.getsockname()[1]
        host_key, client_key = paramiko.RSAKey.generate(2048), paramiko.RSAKey.generate(2048)
        client_key.write_private_key_file(str(root / "identity"))
        (root / "identity").chmod(0o600)
        endpoint = Endpoint(host_key, client_key, game_port)
        known_hosts = root / "known_hosts"
        known_hosts.write_text(f"[127.0.0.1]:{endpoint.port} {host_key.get_name()} {host_key.get_base64()}\n")
        config = root / "config"
        config.write_text(f"""Host termcraft-test
  HostName 127.0.0.1
  User fixture
  IdentityFile {root / 'identity'}
  UserKnownHostsFile {known_hosts}
  GlobalKnownHostsFile /dev/null
  StrictHostKeyChecking yes
  IdentitiesOnly yes
  BatchMode yes
  ControlMaster no
  ConnectTimeout 5
  LogLevel ERROR
""")
        wrapper = root / "bin"
        wrapper.mkdir()
        (wrapper / "ssh").write_text(
            f"#!{sys.executable}\nimport os, pathlib, sys\n"
            "with open(os.environ['TERMCRAFT_TEST_SSH_PIDS'], 'a') as f: f.write(str(os.getpid())+'\\n')\n"
            f"os.execv({ssh!r}, [{ssh!r}, '-F', {str(config)!r}, *sys.argv[1:]])\n")
        (wrapper / "ssh").chmod(0o700)
        env.update(PATH=f"{wrapper}:{os.environ.get('PATH', '/usr/bin:/bin')}",
                   TERMCRAFT_TEST_SSH_PIDS=str(root / "ssh-pids"))
        join = [binary, "join", "termcraft-test", "--ssh-port", str(endpoint.port),
                "--game-port", str(game_port)]
        log = (root / "server.log").open("w")

        def start_server():
            process = subprocess.Popen([binary, "world", "serve", "ssh", f"127.0.0.1:{game_port}"],
                                       env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)

            def ready():
                assert process.poll() is None, "fixture game server exited"
                try:
                    with socket.create_connection(("127.0.0.1", game_port), timeout=0.1):
                        return True
                except OSError:
                    return False

            wait_for(ready)
            return process

        def connect(profile):
            client = Client([*join, "--player", profile], env)
            clients.append(client)
            wait_for(client.pump)
            return client

        def profiles():
            return {p.name: p.read_bytes() for p in (root / "saves" / "clients").glob("*.json")}

        def rejected(profile, expected):
            result = subprocess.run([*join, "--player", profile], env=env, capture_output=True, timeout=10)
            assert result.returncode != 0, "invalid join unexpectedly succeeded"
            assert expected in result.stderr, f"unexpected fixture failure: {result.stderr.decode(errors='replace')[:1000]}"
            assert b"\x1b[?1049h" not in result.stdout, "failed join entered raw game UI"

        try:
            server = start_server()
            alice, bob = connect("alice"), connect("bob")
            initial = profiles()
            assert len(initial) == 2
            rejected("alice", b"already connected")
            alice.resize(200, 60)
            os.kill(alice.process.pid, signal.SIGWINCH)
            alice.quit()
            bob.quit()
            alice = connect("alice")
            assert profiles() == initial, "new SSH tunnel changed reconnect credentials"
            alice.quit()
            server.send_signal(signal.SIGINT)
            assert server.wait(timeout=8) == 0
            server = start_server()
            alice = connect("alice")
            assert profiles() == initial, "server restart changed identity"
            alice.quit()
            endpoint.mode = "deny-auth"
            rejected("auth-failure", b"Permission denied")
            endpoint.mode = "deny-forward"
            rejected("forward-failure", b"SSH game connection failed")
            endpoint.mode = "allow"
            good_hosts = known_hosts.read_text()
            wrong = paramiko.RSAKey.generate(2048)
            known_hosts.write_text(f"[127.0.0.1]:{endpoint.port} {wrong.get_name()} {wrong.get_base64()}\n")
            rejected("host-key-failure", b"Host key verification failed")
            known_hosts.write_text(good_hosts)
            alice = connect("alice")
            for transport in endpoint.transports:
                transport.close()
            wait_for(lambda: (alice.pump(), alice.process.poll() is not None)[1])
            alice.drain()
            assert alice.process.returncode != 0
            assert alice.before == termios.tcgetattr(alice.slave)
            assert b"\x1b[?1049l" in alice.output
            server.send_signal(signal.SIGINT)
            assert server.wait(timeout=8) == 0
            rejected("missing-server", b"SSH game connection failed")
            assert profiles() == initial, "failed handshake wrote a player identity"
            endpoint.close()
            wait_for(lambda: all(not process_exists(pid)
                                 for pid in (root / "ssh-pids").read_text().splitlines()))
            report = dict(status="passed", openssh=subprocess.check_output([ssh, "-V"], stderr=subprocess.STDOUT,
                                                                          text=True).strip(),
                          paramiko=paramiko.__version__, binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                          encrypted_key_authenticated_sessions=endpoint.encrypted,
                          forwarded_channels=endpoint.forwarded, reconnect_and_restart=True,
                          rejected_authentication=True, rejected_forwarding=True, rejected_host_key=True,
                          missing_game_server=True, dropped_link_restores_terminal=True,
                          tunnel_processes_reaped=True, duration_seconds=round(time.monotonic() - started, 3),
                          limits="Local OpenSSH-to-Paramiko fixture. No real remote account, WAN or human gameplay qualification.")
            assert endpoint.authenticated == endpoint.encrypted and endpoint.encrypted >= 5
            if args.report:
                args.report.write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(report))
        finally:
            for client in clients:
                client.close()
            endpoint.close()
            if server and server.poll() is None:
                server.kill()
                server.wait(timeout=3)
            log.close()


if __name__ == "__main__":
    main()
