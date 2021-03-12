# handins_grade
a small program to automatically determine your grade on the handins server

## build + run

```bash
$ git clone https://github.com/soulwa/handins_grade.git
$ cd handins_grade
$ cargo run
```

the program will prompt you with your username and password, which are used to login to the handins server remotely, then disposed of. it will output all of your most recent grades, as well as a (correct) current grade indicator. this is calculated by computing the sum of all finished grades times their weights, divided by the total weights so far. this gives you an accurate score out of 100.