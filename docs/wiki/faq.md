# FAQ

Status: some answers describe features coming in v0.1.

**Why not Tor?**
Tor and I2P solve different problems. Tor Browser is excellent for the clearnet. eepview is for I2P sites only. Use the right tool for each.

**Why only I2P?**
One job is easier to make safe. Every request that is not for an `.i2p` host is refused. There is no mode that mixes the two.

**Does eepview include a router?**
No. It finds a router on your computer. If it finds none, it offers to install Java I2P after you agree.

**Does it update I2P?**
Only a router that eepview installed. Updates arrive over I2P, are signed, and apply at the next start, with a backup first. A router you run yourself is never touched.

**Does it update Java?**
It shows a notice. It downloads nothing before you click. The weekly check is a permission you can turn off.

**What data does it store, and where?**
Bookmarks, history and settings, in your app data folder. Cookies and cache vanish at quit unless you keep them. A managed router keeps its own state in the same area. Nothing is sent anywhere.

**How do I reset it?**
Quit eepview. Delete its app data folder. Settings has "Remove router and all its data" for a router that eepview installed. (Coming in v0.1.)

**Is it affiliated with the I2P project?**
No. eepview is an independent project. It uses the I2P network.

**Can I open normal websites?**
No. They show the blocked page.

**Can I download files?**
Not yet. Downloads are refused for now.

**Is it ready to use?**
No. It is pre-release. See the [roadmap](roadmap.md).

**Where do I report a bug or a vulnerability?**
Bugs go to the issue tracker. Vulnerabilities go to the private channels in [SECURITY.md](https://github.com/tcivie/eepview/blob/main/SECURITY.md).
