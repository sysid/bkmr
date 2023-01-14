# rs-twbm

- panics on null values in DB, but there should'nt be any
- script provided for finding and cleaning

## Benchmarking
- -20x faster than the original after warming up Python.
```bash
time RTWBM_DB_URL=/Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 /Users/Q187392/dev/s/private/rs-twbm/twbm/target/release/twbm search zzzeek --np
1. zzzeek : Asynchronous Python and Databases [345]
   https://techspot.zzzeek.org/2015/02/15/asynchronous-python-and-databases/
   async knowhow py


real    0m0.007s
user    0m0.002s
sys     0m0.002s
-----------------------------------------------------------
time TWBM_DB_URL=sqlite://///Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 /Users/Q187392/.local/bin/twbm search zzzeek --np
0. zzzeek : Asynchronous Python and Databases [345]
   https://techspot.zzzeek.org/2015/02/15/asynchronous-python-and-databases/
   async, knowhow, py


Found: 1
345

real    0m0.396s
user    0m0.215s
sys     0m0.056s


# Coldstart
real    0m0.152s
user    0m0.007s
sys     0m0.017s

real    0m4.606s
user    0m0.681s
sys     0m0.345s
```
# bkmr
