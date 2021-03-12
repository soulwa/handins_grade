# handins_grade
a small program to automatically determine your grade on the handins server

## run standalone
Unix:
```bash
$ ./handins_grade
```

Windows:
```bash
$ handins_grade.exe
``` 

## build + run

```bash
$ git clone https://github.com/soulwa/handins_grade.git
$ cd handins_grade
$ cargo run
```

if running from git bash, run the following:
```bash
$ winpty ./handins_grade
```

as otherwise the terminal will not behave properly when attempting to read your password.

the program will prompt you with your username and password, which are used to login to the handins server remotely, then disposed of. it will output all of your most recent grades, as well as a (correct) current grade indicator. this is calculated by computing the sum of all finished grades times their weights, divided by the total weights so far. this gives you an accurate score out of 100.