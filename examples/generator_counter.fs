counter = *(start: Number, end: Number): Sequence<Number> => {
  yield start
  yield start + 1
  yield end
}

values = counter(3, 5)
