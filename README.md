# Calc

Simple calculator for back-of-envolope estimations and budgeting. 

It accepts a text file with in a simple line oriented format:

```
1000 initial values with optional descriptions
[10, 50] can provide intervals
100 / 12 along with scalar division and multiplication
---------
          a subtotal can be requested by adding a dotted line followed by a blank one

```

`calc` then figures out the subtotals and prints a formatted version of the file back out:

```
            1000 initial values with optional descriptions
        [10, 50] can provide intervals
        100 / 12 along with scalar division and multiplication
----------------
[941.67, 981.67] a subtotal can be requested by adding a dotted line followed by a blank one
```

## Future Features

I don't forsee a need to add any major features but a few ideas I have are:

- Probabilistic Costs: allow intervals to be from a set of common distributions and output a distribution then.
- Inplace Editing: an optional flag to replace files in place
- Embedded syntax: allow `calc` to parse out blocks of code located in markdown files and potentially update them inplace without touching the rest of the file.
- Fixed parser erorrs