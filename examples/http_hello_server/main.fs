import Http from 'std:http'
import { route } from './router.fs'

server = Http.serve({
  host: '127.0.0.1',
  port: 8080,
  maxRequests: 0,
}, route)
