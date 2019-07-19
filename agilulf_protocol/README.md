# Agilulf KV Protocol

The design of this protocol learns from [Redis Protocol](https://redis.io/topics/protocol). But as 
this KV server doesn't provide such complicated data structure, the protocol is much simpler.

This protocol can be used on TCP stream. As it doesn't handle transmission error and doesn't provide
any guarantee on the correctness (by checksum or some other technology), so you have to provide a stable
and secure transmission layer (TCP is quite good for this. Maybe QUIC or KCP can provide better 
performance)

As simpleness is the first guide of this project, ~~this crate implements only serialize and deserialize on
TCP stream. But with some helper function in this crate (they are private now), binding this protocol on 
other transmission layer is not difficult~~.This crate provides interface to accept any `AsyncWrite + Unpin` and 
`AsyncRead + Unpin` and convert them into `Stream<Command>` and `Sink<Reply>`. Agilulf KV server is designed to use latest rust async/await 
feature, so [Stream](https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.17/futures/stream/trait.Stream.html) 
and [Sink](https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.17/futures/sink/trait.Sink.html) 
from `futures` crate is used as interface.

## Request-Response model

The server response to every single request from client. However, client can send several requests 
and wait for response of them at the same time. In this case, the order of response will be consistent
with the order of request.

## Request

This crate deserialized a request with these steps:

1, Check whether the first line (end with `\r\n`) is "*"+number. This number provides information about
How many parts are there in this message.

2. For every parts, it starts with a line "$" + number. This number provides information about how long 
the content of this part will be.

3. Then we get a list of parts. Commands can be read from them easily.

### Example

1. Simple PUT request:

```
*3
$3
PUT
$5
HELLO
$5
WORLD
```

**Note: every line in example is broken with \r\n**, so the request above is actually
`*3\r\n$3\r\nPUT\r\n$5HELLO\r\n$5WORLD`

2. Simple GET request

```
*2
$3
GET
$5
HELLO
```

3. Delete request:

```
*2
$6
DELETE
$5
HELLO
```

4. Scan request:

```
*3
$4
SCAN
$1
A
$6
AAAAAA
```

### Note

This protocol allows to store any binary in content (both key and value). As it gives the length of every 
part, it will never decode the content of every part but only simply copy them into ram.

## Response

Response is much simpler. This protocol doesn't give response form for every type of requests, but only 
provides some simple form (which is enough for a KV server). There are only four types of responses: 

1. Status. The response will start with "+" and following a status message. Such as "+OK" indicates this
request operates successfully. A PUT request and a DELETE request may lead to this type of response.

2. Err. This type of response will start with "-" and following an err message. Such as "-KeyNotFound"
indicates Not Found error. GET a not existing key (or deleted key) will lead to this.

3. One Slice. It starts with the first line "$" + number. The number here tells client how long 
the slice is. And the following is the content of response. For example, a simple GET request may be 
responded with "$5\r\nWORLD\r\n"

4. Multiple Slice. It starts with the first line "*" + number. This number tells client how many slices 
it contains. Then for every slice, the protocol is the same as one slice form. For example, a SCAN request
may be responded with "*1\r\n$5\r\nWORLD\r\n"

## Interface Design

The stream of data flows like this: `TcpStream -> Stream<Command> -> Database -> Sink<Reply> -> TcpSink`. 
This crate pipe the first part and the last part of this flow. It can simply transfer a TcpStream into
Command Stream, and transfer the TcpSink into Reply Sink.

With this crate, implementing a Agilulf KV server only needs to handle the core part: Transform every 
Command into a Reply (respond to every request).

**Note: simply pipe stream to sink will not work. A recv and send loop is needed for this.** I am sorry
 for that I don't know why it occurs. When I try to use `send_all` to pipe these together, I noticed that
 my `Sink<Reply>` has received and handled reply but it never send it out.