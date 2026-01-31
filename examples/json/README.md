# JSON Examples

Demonstrates JSON parsing, building, and validation patterns.

## Examples

| File | Description |
|------|-------------|
| `parse.cov` | Parse JSON and extract fields |
| `build.cov` | Build JSON structures from data |
| `validation.cov` | Validate JSON syntax and round-trip |

## Key Concepts

### Parsing JSON

```covenant
step id="s1" kind="call"
  fn="json.parse"
  arg name="input" from="json_str"
  as="result"
end
```

### Field Extraction

Navigate objects with `json.get_field`:

```covenant
step id="s2" kind="call"
  fn="json.get_field"
  arg name="obj" from="result"
  arg name="key" lit="user"
  as="user_obj"
end
```

### Nested Navigation

Chain calls to navigate deep structures:

```covenant
// Navigate: result.user.profile.displayName
step id="s2" kind="call"
  fn="json.get_field"
  arg name="obj" from="result"
  arg name="key" lit="user"
  as="user_obj"
end

step id="s3" kind="call"
  fn="json.get_field"
  arg name="obj" from="user_obj"
  arg name="key" lit="profile"
  as="profile_obj"
end

step id="s4" kind="call"
  fn="json.get_field"
  arg name="obj" from="profile_obj"
  arg name="key" lit="displayName"
  as="name_field"
end
```

### Array Indexing

```covenant
step id="s2" kind="call"
  fn="json.get_index"
  arg name="arr" from="result"
  arg name="index" lit=0
  as="first_elem"
end
```

### Type Conversion

Extract typed values with `json.as_string`, `json.as_int`, etc.:

```covenant
step id="s5" kind="call"
  fn="json.as_string"
  arg name="value" from="name_field"
  as="name"
end
```

### Validation

Check syntax before parsing:

```covenant
step id="s1" kind="call"
  fn="json.is_valid"
  arg name="input" from="input"
  as="is_valid"
end
```

### Building JSON

Construct structs that serialize to JSON:

```covenant
step id="s1" kind="construct"
  type="ApiResponse"
  field name="status" from="status"
  field name="message" from="message"
  as="response"
end
```
