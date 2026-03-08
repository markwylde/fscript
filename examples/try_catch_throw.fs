answer = try {
  throw {
    tag: 'boom',
    message: 'recovered',
  }
} catch ({ message }) {
  message
}
