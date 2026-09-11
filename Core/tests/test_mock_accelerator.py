import asyncio
import unittest

from Core.hardware_accelerator_interface import MockHardwareAccelerator
from Core.sovereign_code_gate import SovereignCodeGate


class MockAcceleratorIntegrationTest(unittest.TestCase):
    def test_mock_accelerator_proof_pipeline(self) -> None:
        gate = SovereignCodeGate(accelerator=MockHardwareAccelerator())
        proof_ids = [f"proof-{idx}" for idx in range(5000)]

        results = asyncio.run(gate.validate_proofs_async(proof_ids))

        self.assertEqual(len(results), len(proof_ids))
        self.assertTrue(all(result.valid for result in results))

        metrics = gate.last_metrics
        self.assertIsNotNone(metrics)
        assert metrics is not None
        self.assertGreater(metrics.throughput_per_second, 250_000)
        self.assertAlmostEqual(metrics.resonance_hz, 117.0, places=1)


if __name__ == "__main__":
    unittest.main()
