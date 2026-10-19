import os
from typing import List

LIMIT = 10


class Rows(Base):
    def top(self, value: int) -> List[int]:
        """Docs."""
        low = min(value, 3)
        if low < LIMIT and self.rows:
            return [r for r in self.rows if r > low]
        return None  # none


def pick(xs, limit=3):
    return sorted(xs, key=lambda r: r.type, reverse=True)[:limit]
