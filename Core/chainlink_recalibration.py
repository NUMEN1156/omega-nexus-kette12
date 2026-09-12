from __future__ import annotations

from dataclasses import dataclass, replace


TARGET_RESONANCE_HZ = 117.0
MAX_ACCEPTABLE_ENTROPY = 0.98
MIN_RESPONSE_CONSISTENCY = 0.995


@dataclass(frozen=True, slots=True)
class NodeEvent:
    day: int
    summary: str
    severity: float
    kind: str = "telemetry"


@dataclass(frozen=True, slots=True)
class SovereignNode:
    node_id: str
    status: str
    resonance_hz: float
    feed_latency_ms: float
    response_consistency: float
    shield_active: bool
    entropy: float
    yield_agst: float
    recent_events: tuple[NodeEvent, ...] = ()


@dataclass(frozen=True, slots=True)
class NodeDiagnostic:
    node_id: str
    resonance_drift_hz: float
    feed_latency_ms: float
    response_consistency: float
    shield_active: bool
    entropy: float
    yield_agst: float
    recent_alert_event_count: int
    requires_shield_review: bool
    requires_entropy_review: bool


@dataclass(frozen=True, slots=True)
class RecalibrationStep:
    day: int
    node_id: str
    phase: str
    details: str


@dataclass(frozen=True, slots=True)
class MonitoringSnapshot:
    day: int
    node_id: str
    resonance_hz: float
    entropy: float
    yield_agst: float
    status: str


@dataclass(frozen=True, slots=True)
class RecalibrationPlan:
    isolated_node_ids: tuple[str, ...]
    diagnostics: tuple[NodeDiagnostic, ...]
    priority_order: tuple[str, ...]
    recalibrated_nodes: tuple[SovereignNode, ...]
    steps: tuple[RecalibrationStep, ...]
    monitoring_schedule: tuple[MonitoringSnapshot, ...]
    projected_average_resonance_hz: float
    projected_average_entropy: float
    projected_stable_nodes: int
    success_target_met: bool


class ChainlinkExternRecalibrator:
    """Builds a deterministic remediation plan for ALERT nodes."""

    def __init__(
        self,
        target_resonance_hz: float = TARGET_RESONANCE_HZ,
        max_entropy: float = MAX_ACCEPTABLE_ENTROPY,
        min_consistency: float = MIN_RESPONSE_CONSISTENCY,
    ) -> None:
        self.target_resonance_hz = target_resonance_hz
        self.max_entropy = max_entropy
        self.min_consistency = min_consistency

    def build_plan(self, nodes: list[SovereignNode], monitoring_days: int = 7) -> RecalibrationPlan:
        alert_nodes = [node for node in nodes if node.status.upper() == "ALERT"]
        stable_nodes = [node for node in nodes if node.status.upper() != "ALERT"]
        isolated_nodes = [replace(node, status="ISOLATED") for node in alert_nodes]
        diagnostics = tuple(self._diagnose(node) for node in isolated_nodes)
        prioritized = tuple(sorted(diagnostics, key=self._priority_key))
        recalibrated = tuple(self._recalibrate(node, diagnostics) for node in isolated_nodes)
        recalibrated_by_id = {node.node_id: node for node in recalibrated}
        reordered_recalibrated = tuple(recalibrated_by_id[diagnostic.node_id] for diagnostic in prioritized)
        fleet = stable_nodes + list(reordered_recalibrated)
        steps = self._build_steps(prioritized, recalibrated_by_id)
        monitoring = self._build_monitoring_schedule(reordered_recalibrated, monitoring_days)
        avg_resonance = sum(node.resonance_hz for node in fleet) / max(len(fleet), 1)
        avg_entropy = sum(node.entropy for node in fleet) / max(len(fleet), 1)
        stable_count = sum(1 for node in fleet if node.status == "STABLE")
        success_target_met = (
            len(fleet) == len(nodes)
            and stable_count == len(nodes)
            and avg_resonance >= self.target_resonance_hz
            and avg_entropy <= self.max_entropy
        )
        return RecalibrationPlan(
            isolated_node_ids=tuple(node.node_id for node in isolated_nodes),
            diagnostics=prioritized,
            priority_order=tuple(item.node_id for item in prioritized),
            recalibrated_nodes=reordered_recalibrated,
            steps=steps,
            monitoring_schedule=monitoring,
            projected_average_resonance_hz=avg_resonance,
            projected_average_entropy=avg_entropy,
            projected_stable_nodes=stable_count,
            success_target_met=success_target_met,
        )

    def _diagnose(self, node: SovereignNode) -> NodeDiagnostic:
        recent_alert_events = sum(
            1
            for event in node.recent_events
            if event.day <= 7 and (event.kind.upper() == "ALERT" or event.severity >= 0.6)
        )
        return NodeDiagnostic(
            node_id=node.node_id,
            resonance_drift_hz=self.target_resonance_hz - node.resonance_hz,
            feed_latency_ms=node.feed_latency_ms,
            response_consistency=node.response_consistency,
            shield_active=node.shield_active,
            entropy=node.entropy,
            yield_agst=node.yield_agst,
            recent_alert_event_count=recent_alert_events,
            requires_shield_review=not node.shield_active,
            requires_entropy_review=node.entropy > self.max_entropy,
        )

    def _priority_key(self, diagnostic: NodeDiagnostic) -> tuple[float, int, int, float, str]:
        return (
            -diagnostic.resonance_drift_hz,
            -int(diagnostic.requires_shield_review or diagnostic.requires_entropy_review),
            -diagnostic.recent_alert_event_count,
            -diagnostic.feed_latency_ms,
            diagnostic.node_id,
        )

    def _recalibrate(
        self,
        node: SovereignNode,
        diagnostics: tuple[NodeDiagnostic, ...],
    ) -> SovereignNode:
        diagnostic = next(item for item in diagnostics if item.node_id == node.node_id)
        tuned_resonance = max(self.target_resonance_hz, node.resonance_hz + diagnostic.resonance_drift_hz + 0.05)
        tuned_latency = max(1.0, node.feed_latency_ms * 0.85)
        tuned_consistency = max(node.response_consistency, self.min_consistency)
        tuned_entropy = min(node.entropy, self.max_entropy)
        tuned_yield = max(node.yield_agst, 109.29)
        return replace(
            node,
            status="STABLE",
            resonance_hz=round(tuned_resonance, 3),
            feed_latency_ms=round(tuned_latency, 3),
            response_consistency=round(tuned_consistency, 5),
            shield_active=True,
            entropy=round(tuned_entropy, 3),
            yield_agst=round(tuned_yield, 3),
        )

    def _build_steps(
        self,
        prioritized: tuple[NodeDiagnostic, ...],
        recalibrated_by_id: dict[str, SovereignNode],
    ) -> tuple[RecalibrationStep, ...]:
        steps: list[RecalibrationStep] = []
        for day, diagnostic in enumerate(prioritized, start=1):
            steps.extend(
                [
                    RecalibrationStep(day, diagnostic.node_id, "isolate", "Node is logically isolated from the stable fleet."),
                    RecalibrationStep(day, diagnostic.node_id, "diagnose", "Measure resonance drift, feed latency, response consistency, shield state, and compare the last 7 days of events."),
                    RecalibrationStep(day, diagnostic.node_id, "fine_tune", "Adjust resonance parameters and harden shield/entropy controls."),
                    RecalibrationStep(day, diagnostic.node_id, "load_test", "Run a short-load verification before release."),
                    RecalibrationStep(
                        day,
                        diagnostic.node_id,
                        "reintegrate",
                        f"Rejoin the fleet after a clean observation window at {recalibrated_by_id[diagnostic.node_id].resonance_hz} Hz.",
                    ),
                ]
            )
        return tuple(steps)

    def _build_monitoring_schedule(
        self,
        recalibrated_nodes: tuple[SovereignNode, ...],
        monitoring_days: int,
    ) -> tuple[MonitoringSnapshot, ...]:
        schedule: list[MonitoringSnapshot] = []
        for day in range(1, monitoring_days + 1):
            for node in recalibrated_nodes:
                schedule.append(
                    MonitoringSnapshot(
                        day=day,
                        node_id=node.node_id,
                        resonance_hz=node.resonance_hz,
                        entropy=node.entropy,
                        yield_agst=node.yield_agst,
                        status=node.status,
                    )
                )
        return tuple(schedule)
