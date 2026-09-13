"""Fixed motor interface driven only by normalized postsynaptic evidence."""

import math
from dataclasses import asdict, dataclass


@dataclass(frozen=True)
class MotorConfig:
    explore_speed: float = 0.52
    retreat_speed: float = 0.85
    retreat_decisions: int = 8
    search_decisions: int = 35
    search_commit_decisions: int = 4
    enter_threshold: float = 0.08
    release_threshold: float = 0.03
    red_dominance: float = 1.25
    clear_decisions: int = 3
    approach_half_response: float = 0.25

    def __post_init__(self):
        counts = (
            self.retreat_decisions,
            self.search_decisions,
            self.search_commit_decisions,
            self.clear_decisions,
        )
        values = (
            self.explore_speed,
            self.retreat_speed,
            self.enter_threshold,
            self.release_threshold,
            self.red_dominance,
            self.approach_half_response,
        )
        if (
            any(type(v) is not int or v < 1 for v in counts)
            or any(type(v) not in (int, float) or not math.isfinite(v) for v in values)
            or not 0 < self.explore_speed <= 1
            or not 0 < self.retreat_speed <= 1
            or not 0 <= self.release_threshold < self.enter_threshold
            or self.red_dominance < 1
            or self.approach_half_response <= 0
        ):
            raise ValueError("Invalid motor profile")


@dataclass
class MotorState:
    heading: int = 1
    mode: str = "search"
    remaining: int = 0
    armed: bool = True
    clear: int = 0
    search_age: int = 0
    steer: float = 0.0

    def step(self, approach, avoidance, cfg):
        threat = (
            avoidance > cfg.enter_threshold and avoidance > approach * cfg.red_dominance
        )
        self.clear = (
            min(self.clear + 1, cfg.clear_decisions)
            if avoidance < cfg.release_threshold
            else 0
        )
        if self.clear == cfg.clear_decisions:
            self.armed = True
        if self.remaining == 0:
            if threat and self.armed:
                self.heading *= -1
                self.mode = "retreat"
                self.remaining = cfg.retreat_decisions
                self.armed = False
                self.search_age = 0
            elif threat:
                self.mode = "hold"
            elif approach > cfg.enter_threshold and approach >= avoidance:
                self.mode = "approach"
                self.search_age = 0
            else:
                self.mode = "search"
                self.search_age += 1
                if self.search_age >= cfg.search_decisions:
                    self.heading *= -1
                    self.remaining = cfg.search_commit_decisions
                    self.search_age = 0
        if self.mode == "retreat":
            speed = cfg.retreat_speed
        elif self.mode == "hold":
            speed = 0.0
        elif self.mode == "approach":
            speed = cfg.explore_speed + (1 - cfg.explore_speed) * approach / (
                approach + cfg.approach_half_response
            )
        else:
            speed = cfg.explore_speed
        self.remaining = max(self.remaining - 1, 0)
        # The body acceleration cap is the single velocity-smoothing stage.
        self.steer = self.heading * speed
        return self.steer

    def snapshot(self):
        return asdict(self)

    @classmethod
    def restore(cls, state, cfg):
        if not isinstance(state, dict) or set(state) != set(cls.__dataclass_fields__):
            raise ValueError("Invalid motor checkpoint")
        s = cls(**state)
        if (
            type(s.heading) is not int
            or s.heading not in (-1, 1)
            or s.mode not in ("search", "approach", "retreat", "hold")
            or type(s.armed) is not bool
            or type(s.remaining) is not int
            or not 0
            <= s.remaining
            < max(cfg.retreat_decisions, cfg.search_commit_decisions)
            or (s.remaining and s.mode not in ("retreat", "search"))
            or type(s.clear) is not int
            or not 0 <= s.clear <= cfg.clear_decisions
            or type(s.search_age) is not int
            or not 0 <= s.search_age < cfg.search_decisions
            or type(s.steer) not in (float, int)
            or not math.isfinite(s.steer)
            or abs(s.steer) > 1
        ):
            raise ValueError("Invalid motor checkpoint")
        return s
