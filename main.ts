import {scanFiles} from '.'

scanFiles('/run/media/ovenoboyo/Slow Disk/Music/Playlist/Chikki - 2/South of the Border (feat. Camila Cabello & Cardi B).flac', "./out", "/home/ovenoboyo/.config/moosync/databases/songs.db", (err, res) => {
  console.log(err, res)
}, (err, res) => {
  console.log('got playlist', err, res)
})