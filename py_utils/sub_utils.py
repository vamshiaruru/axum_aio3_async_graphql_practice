from time import time


def calculate_factorial(number: int) -> int:
    import numpy

    start = time()
    factorial = 1
    for i in range(1, number):
        factorial = factorial * i
    print(f"Time taken: {(time() - start) * 1000 * 1000} micro seconds")
    return factorial
