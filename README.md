# Keyrock technical assignment. #

The assignment is to connect to two exchanges (bitstamp and binance) and create a small grpc server that will publish the top 10 combined levels along with the spread.


### Status:
**Working** - But lots of room for improvement.


#### Todos/thoughts:
- Better choice of stream: Updates would properly arrive sooner if the L3 feed would be used for bitstamp and the depth diff feed for binance,
- Better error handling, most error handling currently is just to panic.
- Not very generic, could be existent to allow more then just binance and bitstamp.
