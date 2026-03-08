import { json, notFound, text } from './response.fs'

export route = (
  request: { body: String, method: String, path: String }
): { body: String, contentType: String, status: Number } => {
  match (request.path) {
    '/' => text('hello from fscript'),
    '/health' => json({ tag: 'ok' }),
    otherPath => notFound(),
  }
}
