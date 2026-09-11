from __future__ import annotations

from collections import Counter
from dataclasses import dataclass, field
from math import sqrt
from time import time
from typing import Any, Iterable
import re
import uuid

_TOKEN_RE = re.compile(r"[a-zA-Z0-9_]+")


@dataclass(slots=True)
class MemoryTrace:
    identifier: str
    content: str
    metadata: dict[str, Any] = field(default_factory=dict)
    strength: float = 1.0
    created_at: float = field(default_factory=time)
    updated_at: float = field(default_factory=time)
    vector: Counter[str] = field(default_factory=Counter)


class SemanticReasoningBank:
    """Sparse vector memory with cosine similarity and reinforcement decay."""

    def __init__(self) -> None:
        self._memories: dict[str, MemoryTrace] = {}

    @staticmethod
    def _tokenize(text: str) -> list[str]:
        return [token.lower() for token in _TOKEN_RE.findall(text)]

    @classmethod
    def _vectorize(cls, text: str) -> Counter[str]:
        return Counter(cls._tokenize(text))

    @staticmethod
    def _cosine_similarity(left: Counter[str], right: Counter[str]) -> float:
        if not left or not right:
            return 0.0
        common = set(left).intersection(right)
        numerator = sum(left[token] * right[token] for token in common)
        left_norm = sqrt(sum(value * value for value in left.values()))
        right_norm = sqrt(sum(value * value for value in right.values()))
        if left_norm == 0 or right_norm == 0:
            return 0.0
        return numerator / (left_norm * right_norm)

    def remember(self, content: str, metadata: dict[str, Any] | None = None, strength: float = 1.0) -> str:
        identifier = str(uuid.uuid4())
        now = time()
        self._memories[identifier] = MemoryTrace(
            identifier=identifier,
            content=content,
            metadata=dict(metadata or {}),
            strength=strength,
            created_at=now,
            updated_at=now,
            vector=self._vectorize(content),
        )
        return identifier

    def query(self, prompt: str, top_k: int = 3, min_similarity: float = 0.0) -> list[dict[str, Any]]:
        prompt_vector = self._vectorize(prompt)
        ranked: list[dict[str, Any]] = []
        for trace in self._memories.values():
            similarity = self._cosine_similarity(prompt_vector, trace.vector) * max(trace.strength, 0.0)
            if similarity < min_similarity:
                continue
            ranked.append(
                {
                    "id": trace.identifier,
                    "content": trace.content,
                    "metadata": dict(trace.metadata),
                    "strength": trace.strength,
                    "similarity": round(similarity, 4),
                }
            )
        ranked.sort(key=lambda item: item["similarity"], reverse=True)
        return ranked[:top_k]

    def reinforce(self, identifier: str, amount: float = 0.1) -> None:
        trace = self._memories[identifier]
        trace.strength += amount
        trace.updated_at = time()

    def decay(self, identifier: str, amount: float = 0.1) -> None:
        trace = self._memories[identifier]
        trace.strength = max(0.0, trace.strength - amount)
        trace.updated_at = time()

    def remove(self, identifier: str) -> None:
        self._memories.pop(identifier, None)

    def iter_memories(self) -> Iterable[MemoryTrace]:
        return self._memories.values()

    def snapshot(self) -> list[dict[str, Any]]:
        return [
            {
                "id": trace.identifier,
                "content": trace.content,
                "metadata": dict(trace.metadata),
                "strength": round(trace.strength, 3),
                "updated_at": trace.updated_at,
            }
            for trace in self._memories.values()
        ]
