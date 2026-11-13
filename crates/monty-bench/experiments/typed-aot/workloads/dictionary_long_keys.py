# Repeated reads should reuse the existing hash of a heap-string key.
def total(row: dict[str, int], key: str, count: int) -> int:
    result = 0
    for _ in range(count):
        result += row[key]
    return result


key = 'long-key-' * 4096
print(total({key: 7}, key, 2000))
