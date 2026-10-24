"""Reject unsafe host bindings, mounts and runtimes before launching a server."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('termcraft_hosting', Path(__file__).resolve().parents[1] / 'hosting/server.py')
server = importlib.util.module_from_spec(spec)
spec.loader.exec_module(server)


class HostingBoundaries(unittest.TestCase):
    def test_root_and_rootful_engines_are_refused(self):
        with patch.object(server.os, 'geteuid', return_value=0), self.assertRaisesRegex(ValueError, 'without sudo'):
            server.rootless()
        with patch.object(server.os, 'geteuid', return_value=1000), \
                patch.object(server.shutil, 'which', return_value='/usr/bin/podman'), \
                patch.object(server, 'run', return_value=SimpleNamespace(stdout='{"host":{"security":{"rootless":false}}}')), \
                self.assertRaisesRegex(ValueError, 'rootless=true'):
            server.rootless()

    def test_wildcard_public_and_unbounded_configuration_cannot_start_containers(self):
        for address, memory, cpus in [('0.0.0.0', '1g', 2), ('8.8.8.8', '1g', 2),
                                      ('127.0.0.1', '0', 2), ('127.0.0.1', '1g', float('nan'))]:
            args = SimpleNamespace(bind=address, port=25565, memory=memory, cpus=cpus)
            with patch.object(server, 'run') as runtime, self.assertRaises(ValueError):
                server.start(args)
            runtime.assert_not_called()

    def test_missing_resource_controllers_cannot_disable_the_limits(self):
        with patch.object(server.os, 'geteuid', return_value=1000), \
                patch.object(server.shutil, 'which', return_value='/usr/bin/podman'), \
                patch.object(server, 'run', return_value=SimpleNamespace(stdout='{"host":{"security":{"rootless":true},"cgroupVersion":"v2","cgroupControllers":["memory","pids"]}}')), \
                self.assertRaisesRegex(ValueError, 'delegated cpu'):
            server.rootless()

    def test_existing_files_and_symlinks_cannot_become_host_mounts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'personal-file').write_text('private')
            with self.assertRaisesRegex(ValueError, 'unmarked'):
                server.data_directory(root)
            self.assertEqual((root / 'personal-file').read_text(), 'private')
            link = root / 'alias'
            link.symlink_to(root, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, 'symbolic'):
                server.data_directory(link)

    def test_existing_unrelated_container_is_never_managed(self):
        with patch.object(server.subprocess, 'run', return_value=SimpleNamespace(returncode=0, check_returncode=lambda: None)), \
                patch.object(server, 'run', return_value=SimpleNamespace(stdout='[{"Config":{"Labels":{"other":"app"}}}]')), \
                self.assertRaisesRegex(ValueError, 'unrelated container'):
            server.inspect('some-app')


if __name__ == '__main__':
    unittest.main()
