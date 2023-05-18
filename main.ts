import {scanFiles} from '.'

scanFiles('/run/media/ovenoboyo/Slow Disk/Music/test_playlist.m3u', "./out", "/home/ovenoboyo/.config/moosync/databases/songs.db", ",", 12, true, (err, res) => {
  console.log(err, res)
}, (err, res) => {
  console.log('got playlist', err, res)
})