from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable

from .environment import GraphRAGEnvironmentBoard
from .memory import SemanticReasoningBank

LLMCallback = Callable[[list[dict[str, str]], dict[str, Any]], str]


@dataclass(slots=True)
class BrainfishAgent:
    name: str
    specialization: str
    memory_bank: SemanticReasoningBank
    board: GraphRAGEnvironmentBoard
    llm_callback: LLMCallback | None = None
    inherited_context: list[str] = field(default_factory=list)

    def observe(self, event_id: str, summary: str, severity: float = 0.5, **attributes: Any) -> None:
        self.board.add_event(event_id, summary=summary, severity=severity, agent=self.name, **attributes)
        self.memory_bank.remember(summary, metadata={"agent": self.name, "event_id": event_id, **attributes})

    def spawn_specialist(self, name: str, specialization: str) -> "BrainfishAgent":
        inherited = [entry["content"] for entry in self.memory_bank.query(self.specialization, top_k=3)]
        return BrainfishAgent(
            name=name,
            specialization=specialization,
            memory_bank=self.memory_bank,
            board=self.board,
            llm_callback=self.llm_callback,
            inherited_context=inherited,
        )

    def decide(self, prompt: str) -> dict[str, Any]:
        context = self.memory_bank.query(prompt, top_k=3)
        graph_context = self.board.export_context([entry["metadata"].get("event_id", prompt) for entry in context if entry["metadata"].get("event_id")], depth=1)
        if self.llm_callback:
            messages = [
                {"role": "system", "content": f"You are {self.name}, specialized in {self.specialization}."},
                {"role": "user", "content": prompt},
            ]
            response = self.llm_callback(messages, {"context": context, "graph": graph_context})
        else:
            suggestions = "; ".join(entry["content"] for entry in context) or "Collect more evidence before acting."
            response = f"{self.name} recommends: {suggestions}"
        return {
            "agent": self.name,
            "specialization": self.specialization,
            "response": response,
            "memory_hits": context,
            "graph_context": graph_context,
            "inherited_context": list(self.inherited_context),
        }


@dataclass(slots=True)
class LLMMarketPersonaAgent(BrainfishAgent):
    persona: str = "balanced"

    def evaluate_market(self, symbol: str, signals: list[str]) -> dict[str, Any]:
        prompt = f"Assess {symbol} with persona={self.persona}: {'; '.join(signals)}"
        decision = self.decide(prompt)
        bias = {
            "bull": "tilts toward expansion when momentum and liquidity align",
            "bear": "prioritizes downside containment and hedging",
            "risk_officer": "focuses on exposure caps and contagion control",
        }.get(self.persona, "balances upside and resilience")
        decision["response"] = f"{decision['response']} | Persona bias: {bias}."
        decision["symbol"] = symbol
        decision["signals"] = signals
        return decision
