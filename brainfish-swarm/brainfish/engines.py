from __future__ import annotations

from time import time
from typing import Any

from .environment import GraphRAGEnvironmentBoard
from .memory import SemanticReasoningBank


class PredictiveHealingEngine:
    """Projects likely cascades and proposes preventative interventions."""

    def __init__(self, board: GraphRAGEnvironmentBoard, memory_bank: SemanticReasoningBank) -> None:
        self.board = board
        self.memory_bank = memory_bank

    def simulate(self, start_node: str, max_depth: int = 3, risk_floor: float = 0.2) -> list[dict[str, Any]]:
        if start_node not in self.board.graph:
            return []
        start_severity = float(self.board.graph.nodes[start_node].get("severity", 0.5))
        projections = [{"node": start_node, "depth": 0, "risk": round(start_severity, 4), "path": [start_node]}]
        frontier = [(start_node, 0, start_severity, [start_node])]
        while frontier:
            current, depth, risk, path = frontier.pop(0)
            if depth >= max_depth:
                continue
            for _, target, attrs in self.board.graph.out_edges(current, data=True):
                weight = float(attrs.get("weight", 0.5))
                projected_risk = risk * weight
                if projected_risk < risk_floor:
                    continue
                next_path = path + [target]
                projections.append(
                    {
                        "node": target,
                        "depth": depth + 1,
                        "risk": round(projected_risk, 4),
                        "path": next_path,
                        "relation": attrs.get("relation", "influences"),
                    }
                )
                frontier.append((target, depth + 1, projected_risk, next_path))
        projections.sort(key=lambda item: item["risk"], reverse=True)
        return projections

    def propose_interventions(self, start_node: str, max_depth: int = 3) -> list[dict[str, Any]]:
        proposals = []
        for projection in self.simulate(start_node, max_depth=max_depth):
            memory_hits = self.memory_bank.query(projection["node"], top_k=2)
            guidance = memory_hits[0]["content"] if memory_hits else "Isolate node and gather fresh telemetry."
            proposals.append(
                {
                    "target": projection["node"],
                    "risk": projection["risk"],
                    "path": projection["path"],
                    "recommended_action": guidance,
                }
            )
        return proposals


class PruningAxiomEngine:
    """Decays stale or harmful axioms in memory and graph nodes."""

    def __init__(self, board: GraphRAGEnvironmentBoard, memory_bank: SemanticReasoningBank) -> None:
        self.board = board
        self.memory_bank = memory_bank

    def prune(self, max_age_seconds: float = 3600.0, harmful_threshold: float = 0.75) -> dict[str, Any]:
        now = time()
        removed_memories: list[str] = []
        for trace in list(self.memory_bank.iter_memories()):
            harmfulness = float(trace.metadata.get("harmfulness", 0.0))
            age = now - trace.updated_at
            if age >= max_age_seconds or harmfulness >= harmful_threshold or trace.strength <= 0.0:
                removed_memories.append(trace.identifier)
                self.memory_bank.remove(trace.identifier)

        softened_nodes: list[str] = []
        for node_id, attrs in list(self.board.graph.nodes(data=True)):
            updated_at = float(attrs.get("updated_at", now))
            age = now - updated_at
            harmfulness = float(attrs.get("harmfulness", 0.0))
            if harmfulness >= harmful_threshold and age >= max_age_seconds / 4:
                self.board.graph.nodes[node_id]["severity"] = max(0.0, float(attrs.get("severity", 0.0)) - 0.4)
                softened_nodes.append(node_id)
        return {"removed_memories": removed_memories, "softened_nodes": softened_nodes}
