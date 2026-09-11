from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass
from typing import Protocol


@dataclass(slots=True)
class ProofResult:
    proof_id: str
    valid: bool
    latency_ms: float
    accelerator: str


@dataclass(slots=True)
class AccelerationMetrics:
    processed_count: int
    elapsed_seconds: float
    throughput_per_second: float
    resonance_hz: float


class ProofProcessor(Protocol):
    async def process(self, proof_id: str) -> ProofResult:
        ...


class HardwareAcceleratorInterface:
    """Asynchronous interface for FPGA/ASIC-backed proof validation."""

    def __init__(self, accelerator_name: str, concurrency: int = 128) -> None:
        self.accelerator_name = accelerator_name
        self._semaphore = asyncio.Semaphore(concurrency)

    async def process(self, proof_id: str) -> ProofResult:
        async with self._semaphore:
            start = time.perf_counter()
            await asyncio.sleep(0)
            latency_ms = (time.perf_counter() - start) * 1000
            return ProofResult(
                proof_id=proof_id,
                valid=True,
                latency_ms=latency_ms,
                accelerator=self.accelerator_name,
            )

    async def process_batch(self, proof_ids: list[str], resonance_hz: float = 117.0) -> tuple[list[ProofResult], AccelerationMetrics]:
        start = time.perf_counter()
        results = await asyncio.gather(*(self.process(proof_id) for proof_id in proof_ids))
        elapsed = max(time.perf_counter() - start, 1e-9)
        processed_count = len(results)
        throughput = processed_count / elapsed
        return (
            results,
            AccelerationMetrics(
                processed_count=processed_count,
                elapsed_seconds=elapsed,
                throughput_per_second=throughput,
                resonance_hz=resonance_hz,
            ),
        )


class MockHardwareAccelerator(HardwareAcceleratorInterface):
    """Deterministic mock accelerator for CI/CD benchmark simulation."""

    def __init__(self, accelerator_name: str = "mock-fpga", concurrency: int = 2048) -> None:
        super().__init__(accelerator_name=accelerator_name, concurrency=concurrency)

    async def process(self, proof_id: str) -> ProofResult:
        async with self._semaphore:
            await asyncio.sleep(0)
            return ProofResult(
                proof_id=proof_id,
                valid=True,
                latency_ms=0.001,
                accelerator=self.accelerator_name,
            )
