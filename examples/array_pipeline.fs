import Array from 'std:array'

numbers = [1, 2, 3, 4, 5]

result = numbers
  |> Array.map((value: Number): Number => value + 1)
  |> Array.filter((value: Number): Boolean => value > 3)
