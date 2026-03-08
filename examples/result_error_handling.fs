import Number from 'std:number'
import Result from 'std:result'
import String from 'std:string'

type ParseError = {
  tag: 'parse_error',
  message: String,
}

parsePort = (text: String): Result<Number, ParseError> => {
  if (String.isDigits(text)) {
    Result.ok(Number.parse(text))
  } else {
    Result.error({
      tag: 'parse_error',
      message: 'port must contain digits only',
    })
  }
}

parsed = parsePort('8080')
