from typing import Optional

class TickDataPy:
    instrument: str
    last_price: float
    open_price: float
    highest_price: float
    lowest_price: float
    volume: float
    open_interest: float
    timestamp_ms: int

class RawBarPy:
    symbol: str
    open: float
    high: float
    low: float
    close: float
    vol: float
    amount: float
    open_interest: Optional[float]
    delta: Optional[float]

class SignalPy:
    instrument: str
    action: str
    entry: Optional[float]
    stop_loss: Optional[float]
    take_profit: Optional[float]
    size: Optional[float]
    confidence: float
    source: str

class PipelinePy:
    def __init__(self) -> None: ...
    def from_yaml(self, yaml_str: str) -> None: ...
    def status(self) -> str: ...

class ObsBuilder:
    feature_list: list[str]
    def __init__(self, feature_list: list[str]) -> None: ...

class RewardCalculator:
    alpha: float
    beta: float
    gamma: float
    delta: float
    def __init__(self, alpha: float, beta: float, gamma: float, delta: float) -> None: ...
    def calc(
        self,
        log_return: float,
        prev_sharpe: float,
        curr_sharpe: float,
        drawdown_pct: float,
        traded: bool,
        is_holding: bool,
    ) -> float: ...

class TaijiRLEnv:
    def __init__(
        self,
        pipeline: PipelinePy,
        obs_builder: ObsBuilder,
        reward_calc: RewardCalculator,
        total_steps: int = 1000,
        initial_capital: float = 1_000_000.0,
    ) -> None: ...
    def reset(self): ...
    def step(self, action: int): ...
    def render(self) -> None: ...
    def close(self) -> None: ...
