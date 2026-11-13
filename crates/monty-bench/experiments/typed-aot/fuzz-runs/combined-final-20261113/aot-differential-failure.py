events = []
def f(x: int,y: int,data: list[int],row: dict[str,int],n: int):
    a,b,c = x,y,3
    total = 0
    iterations = range(n)
    for i in iterations:
        if ((row['k'] - -6)) % 3 == 0:
            a,b,c = b,c,(((data[-2] * 4) * (-16 ^ row['k']))) % 1000003
        else:
            a,b,c = ((y + y)) % 1000003,a,b
        for item in data:
            total = ((((c | data[-1]) + (c // row['k'])) * data[-3])) % 1000003
        seen = a
    return (a,b,c,total)
try:
    output = ('return',repr(f(-32,13,[1,2,'wrong',4,5],{'k': 6},6)))
except Exception as error:
    output = ('raise',type(error).__name__,str(error))
repr((output,events))