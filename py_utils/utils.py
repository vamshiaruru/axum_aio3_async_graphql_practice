import asyncio
from py_utils.sub_utils import calculate_factorial


def hello() -> str:
    return "World"


def factorial(number: int) -> int:
    print("Calling calculate factorial")
    return calculate_factorial(number)


async def async_op():
    await asyncio.sleep(0.01)
