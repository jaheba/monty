# --- benchmark ---
# Daily revenue in cents, summarized into weekly and rolling monthly reports.
daily = [((day * 7919) % 200000) + 10000 for day in range(200000)]
weekly = [sum(daily[start : start + 7]) for start in range(0, len(daily), 7)]
monthly = [sum(daily[end - 30 : end]) for end in range(30, len(daily) + 1, 7)]
ranked = weekly.copy()
ranked.sort()
assert sum(weekly) == sum(daily)
assert len(monthly) == 28568
print(len(daily), sum(daily), ranked[len(ranked) // 2], sum(monthly))
