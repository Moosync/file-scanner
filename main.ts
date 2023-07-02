import { scanFiles } from ".";

scanFiles(
  "/run/media/ovenoboyo/Slow Disk/Music/",
  "./out",
  "/home/ovenoboyo/.config/moosync/databases/songs.db",
  ",",
  12,
  true,
  (err, res) => {
    console.log("song", err, res);
  },
  (err, res) => {
    console.log("got playlist", err, res);
  },
  () => {
    console.log("ended");
  }
);
