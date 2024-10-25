## Arbitratus

> Aragorn kommer.

![terminal_spit](./docs/img/arbi.png)

`discovery` package "discovers" prices and detects arbitrage opportunities on crypto exchanges.   
If found it sends current price data to an IPC socket.  
`consumer` listens to the IPC socket and consumes messages.  



#### Dev
---

Install [cargo-watch] if not installed, enables `cargo watch` command.

**run `consumer`**
```
cargo watch -x 'run -p consumer'
```

**run `discovery`**
```
cargo watch -x 'run -p discovery'
```


[cargo-watch]: https://github.com/watchexec/cargo-watch


