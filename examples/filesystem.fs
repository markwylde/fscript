import Json from 'std:json'
import FileSystem from 'std:filesystem'

alphaPath = '/tmp/fscript-filesystem-alpha.txt'
betaPath = '/tmp/fscript-filesystem-beta.txt'

writeAlpha = FileSystem.writeFile(alphaPath, 'alpha written eagerly')
writeBeta = FileSystem.writeFile(betaPath, 'beta written eagerly')

alphaExistsBeforeRead = if (FileSystem.exists(alphaPath)) { true } else { false }
betaExistsBeforeRead = if (FileSystem.exists(betaPath)) { true } else { false }

alphaReader = defer FileSystem.readFile(alphaPath)
betaReader = defer FileSystem.readFile(betaPath)

alphaContents = alphaReader + ''
betaContents = betaReader + ''

deleteAlpha = FileSystem.deleteFile(alphaPath)
deleteBeta = FileSystem.deleteFile(betaPath)

alphaExistsAfterDelete = if (FileSystem.exists(alphaPath)) { true } else { false }
betaExistsAfterDelete = if (FileSystem.exists(betaPath)) { true } else { false }

summary = {
  beforeRead: {
    alphaExists: alphaExistsBeforeRead,
    betaExists: betaExistsBeforeRead,
  },
  afterRead: {
    alphaContents: alphaContents,
    betaContents: betaContents,
  },
  afterDelete: {
    alphaExists: alphaExistsAfterDelete,
    betaExists: betaExistsAfterDelete,
  },
}

answer = Json.jsonToPrettyString(summary)
