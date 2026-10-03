"""The refusals `harness/oracle_common.py` makes, one test each.

Run in the `ge-python-oracle` image (or any python3; nothing here needs numpy):

  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 -m unittest discover -s harness -p 'test_oracle_common.py' -v

Every test is a negative control in the sense the gate means it: each one fails if the
refusal it names is deleted, and each names the review finding it pins.
"""

import hashlib
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oracle_common import (  # noqa: E402
    finite,
    networkx_version,
    read_manifest,
    require_cases,
    require_seeds,
)

NAME = "probe"
EMPTY = hashlib.sha256(b"").hexdigest()


def refused(callable_, *args, **kwargs):
    """The message `callable_` refused with, or a failure saying it did not refuse."""
    try:
        callable_(*args, **kwargs)
    except SystemExit as stop:
        return str(stop)
    raise AssertionError("expected a refusal, got a return")


class FixtureDir:
    """A fixtures directory with one arm's jsonl and the manifest that describes it."""

    def __enter__(self):
        self.dir = tempfile.TemporaryDirectory()
        return self

    def __exit__(self, *stop):
        self.dir.cleanup()

    def write(self, body, seeds=1, seal=True):
        path = os.path.join(self.dir.name, f"{NAME}.jsonl")
        with open(path, "w") as handle:
            handle.write(body)
        manifest = {
            "seeds": seeds,
            "fingerprint": "f" * 64,
            "sha256": {f"{NAME}.jsonl": hashlib.sha256(body.encode()).hexdigest() if seal else EMPTY},
        }
        with open(os.path.join(self.dir.name, f"{NAME}-manifest.json"), "w") as handle:
            json.dump(manifest, handle)
        return self.dir.name

    def manifest(self):
        with open(os.path.join(self.dir.name, f"{NAME}-manifest.json")) as handle:
            return json.load(handle)

    def lines(self):
        with open(os.path.join(self.dir.name, f"{NAME}.jsonl")) as handle:
            return handle.readlines()


class ReadManifest(unittest.TestCase):
    """B3: the digest a result records is the one computed over the bytes read."""

    def test_it_returns_the_computed_digest_not_the_manifests(self):
        with FixtureDir() as fixture:
            directory = fixture.write('{"seed":0}\n')
            _, digest = read_manifest(directory, NAME)
        self.assertEqual(digest, hashlib.sha256(b'{"seed":0}\n').hexdigest())

    def test_a_manifest_that_does_not_describe_the_bytes_is_refused(self):
        with FixtureDir() as fixture:
            directory = fixture.write('{"seed":0}\n', seal=False)
            message = refused(read_manifest, directory, NAME)
        self.assertIn(f"{NAME}.jsonl does not match its manifest", message)


class RequireSeeds(unittest.TestCase):
    """M19 / m57: a fixture file that is not the one the manifest counted is refused."""

    def test_a_short_file_is_refused_by_count(self):
        with FixtureDir() as fixture:
            fixture.write('{"seed":0}\n', seeds=1000)
            message = refused(require_seeds, fixture.manifest(), fixture.lines(), NAME)
        self.assertIn("holds 1 cases, want the manifest's 1000 seeds", message)

    def test_the_counted_file_passes(self):
        with FixtureDir() as fixture:
            fixture.write('{"seed":0}\n', seeds=1)
            self.assertIsNone(require_seeds(fixture.manifest(), ["a line"], NAME))


class RequireCases(unittest.TestCase):
    """B4 / M23 / m44: a layout that compared nothing is not a pass."""

    def test_a_layout_with_no_case_is_refused(self):
        layouts = {"ring": {"cases": 0, "worst": 0.0}, "spiral": {"cases": 3, "worst": 0.0}}
        message = refused(require_cases, layouts, ("ring", "spiral"), NAME)
        self.assertIn(f"{NAME}.jsonl compared no ring case", message)

    def test_every_layout_having_a_case_passes(self):
        layouts = {"ring": {"cases": 1, "worst": 0.0}}
        self.assertIsNone(require_cases(layouts, ("ring",), NAME))


class Finite(unittest.TestCase):
    """M22: `max` is false for a NaN, so the accumulator would keep its 0.0 initialiser."""

    def test_a_nan_metric_is_refused(self):
        self.assertIn("non-finite ring gap", refused(finite, float("nan"), "ring gap"))

    def test_an_infinite_metric_is_refused(self):
        self.assertIn("non-finite ring gap", refused(finite, float("inf"), "ring gap"))

    def test_the_check_is_the_one_max_cannot_make(self):
        self.assertEqual(max(0.0, float("nan")), 0.0, "max would have reported a match")

    def test_a_finite_metric_passes_through_as_a_float(self):
        self.assertEqual(finite(3, "ring gap"), 3.0)


class NetworkxVersion(unittest.TestCase):
    """m59: the recorded version was never compared with anything."""

    class Fake:
        def __init__(self, version):
            self.__version__ = version

    def test_the_pinned_version_passes(self):
        from oracle_common import NETWORKX_PIN

        self.assertEqual(networkx_version(self.Fake(NETWORKX_PIN)), NETWORKX_PIN)

    def test_another_version_is_refused(self):
        self.assertIn("want 3.6", refused(networkx_version, self.Fake("3.4")))


if __name__ == "__main__":
    unittest.main()