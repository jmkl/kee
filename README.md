## USAGE UPDATE
```
git clone https://github.com/jmkl/kee.git
cd kee
cargo run --example kee
```


# CONFIG FILE

where T: is for title
without T represent app name without .exe
[CYCLE] is special case where it will cycle all app in `apps` field
```json
{
  "apps": ["T:TSOOGLE", "T:TSOCKEE", "PHOTOSHOP"],
  "kees": {
    "M-1": "app::TSOCKEE(TODO)",
    "M-2": "app::PHOTOSHOP",
    "M-3": "app::TSOCKEE(TSOOGLE)",
    "M-a": "app::CYCLE"
  }
}
```
