import Json from 'std:json'

export text = (body: String): { body: String, contentType: String, status: Number } => {
  {
    body: body,
    contentType: 'text/plain',
    status: 200,
  }
}

export json = (value: Unknown): { body: String, contentType: String, status: Number } => {
  {
    body: Json.stringify(value),
    contentType: 'application/json',
    status: 200,
  }
}

export notFound = (): { body: String, contentType: String, status: Number } => {
  {
    body: 'not found',
    contentType: 'text/plain',
    status: 404,
  }
}
