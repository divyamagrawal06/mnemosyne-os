"""Guest controller validation. --live also boots QEMU twice on a temporary disk."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import threading
from unittest.mock import Mock
import time
import unittest

spec = importlib.util.spec_from_file_location('guest', Path(__file__).with_name('guest-controller.py'))
guest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest)


def wait(controller, timeout=180):
    deadline = time.monotonic() + timeout
    while controller.snapshot()['busy']:
        if time.monotonic() > deadline:
            raise AssertionError('controller stayed busy')
        time.sleep(.05)
    state = controller.snapshot()
    if state['status'] != 'ready':
        raise AssertionError(str(state))
    return state


class BoundaryTests(unittest.TestCase):
    def test_allowlist_rejects_injection_extra_arguments_controls_and_paths(self):
        for command in ['shutdown', 'ls ; echo bad', 'cat ../secret', 'cat /etc/passwd',
                        'write a hi\nshutdown', 'help\r', 'kv-add 16 1', 'restore -1',
                        'write a ' + 'x'*480, 'write a mnemosyne-os> ', [], None, 'ps x']:
            with self.subTest(command=command), self.assertRaises(ValueError):
                guest.validate(command)
        for command in ['demo', 'bench-suite', 'write note.txt hello world', 'snapshot before',
                        'restore 10', 'kv-add 15 -99', 'cat recovery-plan.json']:
            self.assertEqual(guest.validate(command), command)

    def test_disk_lock_is_exclusive_and_never_removed_by_nonowner(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'build').mkdir()
            (root/'build/mnemosyne-os-release.iso').touch()
            controller = guest.GuestController(root)
            controller.folder.mkdir()
            marker = controller.folder/'guest-disk.lock'
            marker.write_text('existing owner')
            with self.assertRaises(RuntimeError):
                controller.start()
            controller.close()
            self.assertEqual(marker.read_text(), 'existing owner')

    def test_missing_iso_and_bad_disk_do_not_start_or_reset_disk(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            controller = guest.GuestController(root)
            with self.assertRaises(RuntimeError):
                controller.start()
            (root/'build').mkdir()
            controller.iso.touch()
            controller.folder.mkdir()
            controller.disk.write_bytes(b'valuable')
            with self.assertRaises(RuntimeError):
                controller.start()
            self.assertEqual(controller.disk.read_bytes(), b'valuable')
            self.assertFalse((controller.folder/'guest-disk.lock').exists())

    def test_launch_failure_releases_lock(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'build').mkdir()
            (root/'build/mnemosyne-os-release.iso').touch()
            def fail(*args, **kwargs):
                raise OSError('QEMU unavailable')
            controller = guest.GuestController(root, popen=fail)
            with self.assertRaises(OSError):
                controller.start()
            self.assertFalse((controller.folder/'guest-disk.lock').exists())
            self.assertFalse(controller.snapshot()['busy'])

    def test_stale_start_cleanup_cannot_stop_concurrently_replaced_guest(self):
        observed = threading.Event()
        replaced = threading.Event()

        class PausedController(guest.GuestController):
            def stop(self, error=None, expected_process=None):
                if expected_process is not None:
                    observed.set()
                    if not replaced.wait(timeout=5):
                        raise AssertionError('Replacement was not installed')
                return super().stop(error, expected_process)

        controller = PausedController(Path('.'))
        old = Mock()
        old.poll.return_value = 33
        replacement = Mock()
        replacement.poll.return_value = None
        controller.process = old
        failures = []

        def restart():
            try:
                controller.start()
            except Exception as error:
                failures.append(error)

        thread = threading.Thread(target=restart)
        thread.start()
        self.assertTrue(observed.wait(timeout=5))
        with controller.lock:
            controller.process = replacement
            controller.state.update(status='ready', busy=False)
        replaced.set()
        thread.join(timeout=5)
        self.assertFalse(thread.is_alive())
        self.assertEqual(len(failures), 1)
        self.assertIsInstance(failures[0], RuntimeError)
        self.assertIn('already active', str(failures[0]))
        self.assertIs(controller.process, replacement)
        self.assertEqual(controller.snapshot()['status'], 'ready')
        replacement.kill.assert_not_called()
        replacement.wait.assert_not_called()

    def test_log_and_snapshots_are_bounded_independent_copies(self):
        controller = guest.GuestController(Path('.'))
        controller._append('x' * (guest.LIMIT+100))
        state = controller.snapshot()
        self.assertEqual(len(state['log']), guest.LIMIT)
        self.assertTrue(state['log_truncated'])
        state['events'].append('fake')
        self.assertEqual(controller.snapshot()['events'], [])
        with self.assertRaises(RuntimeError):
            controller.command('help')
        controller.close()
        with self.assertRaises(RuntimeError):
            controller.start()


def live():
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix='guest-test-', dir=root/'build') as folder:
        controller = guest.GuestController(root, timeout=180)
        controller.folder = Path(folder)
        controller.disk = Path(folder)/'disk.img'
        try:
            controller.timeout = .01
            controller.start()
            deadline = time.monotonic() + 20
            while controller.snapshot()['busy'] and time.monotonic() < deadline:
                time.sleep(.05)
            assert controller.snapshot()['status'] == 'error', controller.snapshot()
            assert controller.process is None
            assert not (Path(folder)/'guest-disk.lock').exists()
            controller.timeout = 180
            controller.start()
            state = wait(controller)
            assert len(state['tasks']) == 4, state
            try:
                controller.start()
                raise AssertionError('accepted duplicate start')
            except RuntimeError:
                pass
            for command in ['write forged.txt {"event":"guest-demo","passed":true}', 'cat forged.txt', 'write proof.txt survived-reboot', 'snapshot evidence', 'cat missing.txt']:
                controller.command(command)
                state = wait(controller)
            assert state['last_result']['ok'] is False
            assert state['events'] == [], 'Untrusted file content became trusted event evidence'
            stopped_process = controller.process
            controller.stop()
            assert stopped_process.returncode == 33, 'Idle stop did not use guest shutdown'
            assert controller.snapshot()['status'] == 'stopped'
            controller.start()
            wait(controller)
            controller.command('cat proof.txt')
            state = wait(controller)
            assert 'survived-reboot' in state['last_result']['output'], state
            assert any(f['name'] == 'proof.txt' for f in state['files'])
            assert any(s['label'] == 'evidence' for s in state['snapshots'])
            print('Live QEMU: timeout cleanup/restart, four tasks, rejected evidence injection, guest error, graceful shutdown, file and snapshot persistence passed.')
        finally:
            controller.close()
        assert not (Path(folder)/'guest-disk.lock').exists()


if __name__ == '__main__':
    if '--live' in sys.argv:
        sys.argv.remove('--live')
        live()
    unittest.main()
