
Here’s a concise and precise overview of how **Jackson JSON serialization/deserialization** works in Java, along with commonly used annotations and their effects.

---

## ✅ Serialization and Deserialization

- **Serialization:** Converting Java objects to JSON.
- **Deserialization:** Converting JSON back into Java objects.

Jackson’s `ObjectMapper` class performs both:

```java
ObjectMapper mapper = new ObjectMapper();

String json = mapper.writeValueAsString(object);      // Serialization
MyClass obj = mapper.readValue(json, MyClass.class);  // Deserialization
```

---

## ✅ Common Jackson Annotations

| Annotation               | Scope            | Effect                                                 | Example usage                    |
|--------------------------|------------------|--------------------------------------------------------|----------------------------------|
| `@JsonProperty`          | Field/Method     | Renames property in JSON                               | `@JsonProperty("user_name")`     |
| `@JsonIgnore`            | Field/Method     | Excludes property from serialization                   | `@JsonIgnore`                    |
| `@JsonInclude`           | Class/Field      | Controls inclusion criteria (e.g. exclude null/empty)  | `@JsonInclude(Include.NON_NULL)` |
| `@JsonIgnoreProperties`  | Class            | Ignores unknown or specified properties                | `@JsonIgnoreProperties(ignoreUnknown=true)` |
| `@JsonPropertyOrder`     | Class            | Specifies order of serialized properties               | `@JsonPropertyOrder({"id","name"})` |
| `@JsonFormat`            | Field/Method     | Formats dates, numbers, or enums                       | `@JsonFormat(pattern="yyyy-MM-dd")` |
| `@JsonCreator`           | Constructor      | Marks constructor/factory method for deserialization   | `@JsonCreator public MyClass(...)` |
| `@JsonAnyGetter`         | Method           | Includes dynamic key-values in serialization           | `@JsonAnyGetter`                 |
| `@JsonAnySetter`         | Method           | Handles unknown properties in deserialization          | `@JsonAnySetter`                 |

---

## ✅ Detailed Examples of Common Annotations

### ① `@JsonProperty`

Rename fields during serialization/deserialization:

```java
@JsonProperty("user_name")
private String userName;
```

Java ↔ JSON:
```json
{"user_name":"alice"}
```

---

### ② `@JsonIgnore`

Exclude from JSON completely:

```java
@JsonIgnore
private String password;
```

The field is neither serialized nor deserialized.

---

### ③ `@JsonInclude`

Exclude null or empty fields:

```java
@JsonInclude(JsonInclude.Include.NON_NULL)
private String description;
```

If `description` is null, JSON omits it completely:
```json
// "description": null  →  omitted from JSON
```

Common options:
- `ALWAYS` (default)
- `NON_NULL`
- `NON_EMPTY` (excludes empty strings, empty lists)
- `NON_DEFAULT`

---

### ④ `@JsonIgnoreProperties`

Ignore unknown properties to avoid exceptions during deserialization:

```java
@JsonIgnoreProperties(ignoreUnknown = true)
public class MyClass { ... }
```

If JSON has extra fields, they're silently ignored.

---

### ⑤ `@JsonFormat`

Date or number formatting control:

```java
@JsonFormat(shape=JsonFormat.Shape.STRING, pattern="yyyy-MM-dd")
private LocalDate birthDate;
```

Serialized as:
```json
{"birthDate":"2025-01-15"}
```

---

### ⑥ `@JsonCreator` & `@JsonProperty`

Define constructor or factory methods for deserialization explicitly:

```java
@JsonCreator
public MyClass(@JsonProperty("id") int id,
               @JsonProperty("name") String name) {
    this.id = id;
    this.name = name;
}
```

---

### ⑦ Dynamic Properties (`@JsonAnyGetter`, `@JsonAnySetter`)

For flexible key-value pairs:

```java
private Map<String, Object> extraProperties = new HashMap<>();

@JsonAnyGetter
public Map<String, Object> getExtraProperties() {
    return extraProperties;
}

@JsonAnySetter
public void setExtraProperty(String key, Object value) {
    extraProperties.put(key, value);
}
```

---

## ✅ Global configuration via ObjectMapper options

You can globally adjust serialization/deserialization behavior via `ObjectMapper`:

```java
ObjectMapper mapper = new ObjectMapper()
    .configure(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false)
    .configure(SerializationFeature.WRITE_DATES_AS_TIMESTAMPS, false)
    .setSerializationInclusion(JsonInclude.Include.NON_NULL);
```

Common features:

- **SerializationFeature**:
  - `INDENT_OUTPUT` (pretty print JSON)
  - `WRITE_DATES_AS_TIMESTAMPS`

- **DeserializationFeature**:
  - `FAIL_ON_UNKNOWN_PROPERTIES`
  - `ACCEPT_SINGLE_VALUE_AS_ARRAY`

---

## ✅ Summary Table of Key Concepts

| Concept                | Description                                                   |
|------------------------|---------------------------------------------------------------|
| **Annotations**        | Customize property serialization/deserialization behaviors.   |
| **ObjectMapper config**| Global configuration for all operations performed by mapper.  |
| **Serialization**      | Java object → JSON                                            |
| **Deserialization**    | JSON → Java object                                            |

---

**Recommended Approach**:
- Annotate Java classes clearly.
- Globally configure the `ObjectMapper` according to project needs.

This results in predictable, robust, and maintainable JSON handling.

Let me know if you need examples for specific scenarios or annotations.
