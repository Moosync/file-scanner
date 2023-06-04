import {scanFiles} from '.'

scanFiles('E:\\Music', "./out", "C:\\Users\\sahil\\AppData\\Roaming\\moosync\\databases\\songs.db", ",", 12, false, (err, res) => {
  if (res?.song?.playlistId)
  console.log('song', err, res.song.playlistId)
}, (err, res) => {
  console.log('got playlist', err, res)
}, () => {
  console.log('ended')
})