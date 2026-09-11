from __future__ import annotations

from collections import deque
from time import time
from typing import Any

import networkx as nx


class GraphRAGEnvironmentBoard:
    """Directed knowledge graph for entities, events, and causal cascades."""

    def __init__(self) -> None:
        self.graph = nx.DiGraph()

    def add_entity(self, node_id: str, kind: str, **attributes: Any) -> None:
        payload = {"kind": kind, "updated_at": time(), **attributes}
        if node_id in self.graph:
            self.graph.nodes[node_id].update(payload)
        else:
            self.graph.add_node(node_id, **payload)

    def add_event(self, event_id: str, summary: str, severity: float = 0.5, **attributes: Any) -> None:
        self.add_entity(event_id, kind="event", summary=summary, severity=severity, **attributes)

    def link(self, source: str, target: str, relation: str, weight: float = 1.0, **attributes: Any) -> None:
        self.graph.add_edge(source, target, relation=relation, weight=weight, updated_at=time(), **attributes)

    def neighborhood(self, node_id: str, depth: int = 2) -> list[dict[str, Any]]:
        if node_id not in self.graph:
            return []
        seen = {node_id}
        queue: deque[tuple[str, int]] = deque([(node_id, 0)])
        context: list[dict[str, Any]] = []
        while queue:
            current, level = queue.popleft()
            context.append({"node": current, "depth": level, "attributes": dict(self.graph.nodes[current])})
            if level >= depth:
                continue
            for neighbor in set(self.graph.predecessors(current)).union(self.graph.successors(current)):
                if neighbor in seen:
                    continue
                seen.add(neighbor)
                queue.append((neighbor, level + 1))
        return context

    def causal_path(self, source: str, target: str) -> list[str]:
        try:
            return nx.shortest_path(self.graph, source=source, target=target)
        except (nx.NetworkXNoPath, nx.NodeNotFound):
            return []

    def export_context(self, focus_nodes: list[str], depth: int = 1) -> dict[str, Any]:
        nodes: dict[str, dict[str, Any]] = {}
        edges: list[dict[str, Any]] = []
        for focus in focus_nodes:
            for entry in self.neighborhood(focus, depth=depth):
                node = entry["node"]
                nodes[node] = entry["attributes"]
        for source, target, attrs in self.graph.edges(data=True):
            if source in nodes and target in nodes:
                edges.append({"source": source, "target": target, **attrs})
        return {"nodes": nodes, "edges": edges}

    def high_risk_nodes(self, threshold: float = 0.7) -> list[dict[str, Any]]:
        risky = []
        for node_id, attrs in self.graph.nodes(data=True):
            severity = float(attrs.get("severity", 0.0))
            if severity >= threshold:
                risky.append({"node": node_id, "severity": severity, "kind": attrs.get("kind")})
        risky.sort(key=lambda item: item["severity"], reverse=True)
        return risky
