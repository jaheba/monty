# Keep distinct strings alive through validation and the RSS measurement.
COUNT = 5000
WIDTH = 4096
prefix = '中文漢字' * ((WIDTH - 8) // 4)
values = []
for index in range(COUNT):
    values.append(prefix + str(index).rjust(8, '0'))
checksum = sum(len(value) for value in values)
assert checksum == COUNT * WIDTH
assert values[0] == prefix + '00000000'
assert values[-1] == prefix + '00004999'
print(len(values), checksum, values[-1][-8:])
