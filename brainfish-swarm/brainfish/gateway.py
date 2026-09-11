from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable

from .agents import BrainfishAgent
from .environment import GraphRAGEnvironmentBoard
from .memory import SemanticReasoningBank


@dataclass(slots=True)
class MacroReportAgent:
    board: GraphRAGEnvironmentBoard
    memory_bank: SemanticReasoningBank

    def summarize(self, focus_nodes: Iterable[str] | None = None) -> dict[str, object]:
        focus = list(focus_nodes or [])
        return {
            "focus": focus,
            "high_risk_nodes": self.board.high_risk_nodes(),
            "context": self.board.export_context(focus, depth=2) if focus else {"nodes": {}, "edges": []},
            "memory_snapshot": self.memory_bank.snapshot(),
        }


@dataclass(slots=True)
class DeepInteractionGateway:
    board: GraphRAGEnvironmentBoard
    memory_bank: SemanticReasoningBank
    agents: dict[str, BrainfishAgent] = field(default_factory=dict)

    def register(self, agent: BrainfishAgent) -> None:
        self.agents[agent.name] = agent

    def ask(self, agent_name: str, prompt: str) -> dict[str, object]:
        agent = self.agents[agent_name]
        return agent.decide(prompt)

    def ask_all(self, prompt: str) -> list[dict[str, object]]:
        return [agent.decide(prompt) for agent in self.agents.values()]
