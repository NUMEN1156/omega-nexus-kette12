from __future__ import annotations

from dataclasses import dataclass, field

from .hardware_accelerator_interface import AccelerationMetrics, HardwareAcceleratorInterface, ProofResult


@dataclass(slots=True)
class SovereignCodeGate:
    accelerator: HardwareAcceleratorInterface
    last_metrics: AccelerationMetrics | None = field(default=None, init=False)

    async def validate_proofs_async(self, proof_ids: list[str]) -> list[ProofResult]:
        results, metrics = await self.accelerator.process_batch(proof_ids)
        self.last_metrics = metrics
        return results
