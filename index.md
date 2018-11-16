## Golem Unlimited

* [cargo doc](https://golemfactory.github.io/golem-unlimited/docs/master/gu_base/index.html)

### First steps

Start local Hub in background
```shell 
$ gu-hub server start                       
```

Now you can access Hub web GUI <http://127.0.0.1:61621/app/index.html>

Start single local Provider in background and connect to the existing local Hub.
```       
$ gu-provider server start
$ gu-provider hubs connect --save 127.0.0.1:61621
```

It should appear in **providers** section of the Hub web GUI.

