# Generator
## Usage
```rs
let gen = Generator::new(|y| {
    y.yield_from(Generator::new(|y2| {
        for i in 0..100 {
            y2.r#yield(i);
        }
    }));

    y.yield_from(0..100);
});

assert!(gen.eq((0..100).chain(0..100)))
```
