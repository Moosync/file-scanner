import {scanFiles} from '.'

scanFiles('/run/media/ovenoboyo/Slow Disk/Music/', "./out", "/home/ovenoboyo/.config/moosync/databases/songs.db", (err, res) => {
  console.log(err, res)
}, (err, res) => {
  console.log('got playlist', err, res)
})

console.log('hello')