# HTTP Server Examples

Demonstrates separating pure routing logic from effectful I/O, a key pattern in Covenant for testability and clarity.

## Examples

| File | Description |
|------|-------------|
| `http-server.cov` | Request routing and server loop |

## Key Concepts

### Pure Routing Logic

The routing function has no effects—it's pure logic that maps requests to responses:

```covenant
snippet id="http.handle_request" kind="fn"

// No effects section = pure function

signature
  fn name="handle_request"
    param name="req" type="Request"
    returns type="Response"
  end
end

body
  // Extract method and path
  step id="s1" kind="bind"
    field="method" of="req"
    as="method"
  end
  step id="s2" kind="bind"
    field="path" of="req"
    as="path"
  end

  // Route matching with if conditions
  step id="s3" kind="compute"
    op=equals
    input var="method"
    input lit="GET"
    as="is_get"
  end
  // ... pattern matching continues
end
```

### Effectful Server Loop

The main function handles I/O effects while delegating routing to the pure function:

```covenant
snippet id="http.main" kind="fn"

effects
  effect http_server
  effect console
end

body
  step id="s1" kind="call"
    fn="listen"
    arg name="port" lit=8080
    as="server"
  end

  step id="s3" kind="call"
    fn="requests"
    arg name="server" from="server"
    as="request_stream"
  end

  step id="s4" kind="for"
    var="req" in="request_stream"
    step id="s4a" kind="call"
      fn="http.handle_request"   // Pure!
      arg name="req" from="req"
      as="response"
    end
    step id="s4b" kind="call"
      fn="respond"
      arg name="req" from="req"
      arg name="response" from="response"
      as="_"
    end
    as="_"
  end
end
```

### Data Types

Request and Response are simple structs:

```covenant
snippet id="http.Request" kind="struct"
signature
  struct name="Request"
    field name="method" type="String"
    field name="path" type="String"
    field name="headers" type="Map<String, String>"
    field name="body" type="String"
  end
end
end
```
