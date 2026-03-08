type User =
  | { tag: 'guest' }
  | { tag: 'member', name: String }

displayName = (user: User): String => match (user) {
  { tag: 'guest' } => 'Guest',
  { tag: 'member', name } => name,
}

currentUser = { tag: 'member', name: 'Grace' }
label = displayName(currentUser)
