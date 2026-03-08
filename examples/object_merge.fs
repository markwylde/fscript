import Object from 'std:object'

baseUser = {
  id: 'user-1',
  name: 'Ada',
}

activeUser = Object.spread(baseUser, {
  active: true,
  role: 'admin',
})
