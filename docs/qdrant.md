# Qdrant Rust client

The [Qdrant](https://qdrant.tech/) - High-Performance Vector Search at Scale - client for Rust.

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![Apache 2.0 licensed][apache2-badge]][apache2-url]

[crates-badge]: https://img.shields.io/crates/v/qdrant-client.svg
[crates-url]: https://crates.io/crates/qdrant-client
[docs-badge]: https://img.shields.io/docsrs/qdrant-client.svg
[docs-url]: https://docs.rs/qdrant-client
[apache2-badge]: https://img.shields.io/badge/license-apache2-blue.svg
[apache2-url]: https://github.com/qdrant/rust-client/blob/master/LICENSE

Documentation:

- Qdrant documentation: <https://qdrant.tech/documentation/>
- Crate documentation: <https://docs.rs/qdrant-client>

## Installation

```bash
cargo add qdrant-client
```

Package is available in [crates.io](https://crates.io/crates/qdrant-client)

## Examples

A list of example snippets can be found [here](https://github.com/qdrant/api-reference/tree/main/snippets/rust)

More examples can be found in the [examples folder](https://github.com/qdrant/rust-client/tree/master/examples)

## Dependencies

The client uses gRPC via the [Tonic](https://github.com/hyperium/tonic) library.

To change anything in the protocol buffer definitions, you need the `protoc` Protocol Buffers compiler, along with Protocol Buffers resource files.

Refer to the [Tonic installation guide](https://github.com/hyperium/tonic#dependencies) for more details.

## Usage

Run Qdrant with enabled gRPC interface:

```bash
# With env variable
docker run -p 6333:6333 -p 6334:6334 \
    -e QDRANT__SERVICE__GRPC_PORT="6334" \
    qdrant/qdrant
```

Or by updating the configuration file:

```yaml
service:
  grpc_port: 6334
```

More info about gRPC in [documentation](https://qdrant.tech/documentation/quick_start/#grpc).

### Making requests

Add necessary dependencies:

```bash
cargo add qdrant-client anyhow tonic tokio serde-json --features tokio/rt-multi-thread
```

Add query example from [`examples/query.rs`](./examples/query.rs) to your `src/main.rs`:

```rust
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, Distance, Filter, PointStruct, QueryPointsBuilder,
    ScalarQuantizationBuilder, SearchParamsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::{Payload, Qdrant, QdrantError};

#[tokio::main]
async fn main() -> Result<(), QdrantError> {
    // Example of top level client
    // You may also use tonic-generated client from `src/qdrant.rs`
    let client = Qdrant::from_url("http://localhost:6334").build()?;

    let collections_list = client.list_collections().await?;
    dbg!(collections_list);
    // collections_list = {
    //   "collections": [
    //     {
    //       "name": "test"
    //     }
    //   ]
    // }

    let collection_name = "test";
    client.delete_collection(collection_name).await?;

    client
        .create_collection(
            CreateCollectionBuilder::new(collection_name)
                .vectors_config(VectorParamsBuilder::new(10, Distance::Cosine))
                .quantization_config(ScalarQuantizationBuilder::default()),
        )
        .await?;

    let collection_info = client.collection_info(collection_name).await?;
    dbg!(collection_info);

    let payload: Payload = serde_json::json!(
        {
            "foo": "Bar",
            "bar": 12,
            "baz": {
                "qux": "quux"
            }
        }
    )
    .try_into()
    .unwrap();

    let points = vec![PointStruct::new(0, vec![12.; 10], payload)];
    client
        .upsert_points(UpsertPointsBuilder::new(collection_name, points))
        .await?;

    let query_result = client
        .query(
            QueryPointsBuilder::new(collection_name)
                .query(vec![11.0; 10])
                .limit(10)
                .filter(Filter::all([Condition::matches("bar", 12)]))
                .with_payload(true)
                .params(SearchParamsBuilder::default().exact(true)),
        )
        .await?;
    dbg!(&query_result);
    // query_result = [
    //   {
    //     "id": 0,
    //     "version": 0,
    //     "score": 1.0000001,
    //     "payload": {
    //       "bar": 12,
    //       "baz": {
    //         "qux": "quux"
    //       },
    //       "foo": "Bar"
    //     }
    //   }
    // ]

    let found_point = query_result.result.into_iter().next().unwrap();
    let mut payload = found_point.payload;
    let baz_payload = payload.remove("baz").unwrap().into_json();
    println!("baz: {baz_payload}");
    // baz: {"qux":"quux"}

    Ok(())
}
```

Or run the example from this project directly:

```bash
cargo run --example search
```

## Qdrant Cloud

[Qdrant Cloud](https://cloud.qdrant.io) is a managed service for Qdrant.

The client needs to be configured properly to access the service.

- make sure to use the correct port (6334)
- make sure to pass your API KEY

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://xxxxxxxxxx.eu-central.aws.cloud.qdrant.io:6334")
    // Use an environment variable for the API KEY for example
    .api_key(std::env::var("QDRANT_API_KEY"))
    .build()?;
```

---

# Get collection details

GET http://localhost:6333/collections/{collection_name}

Retrieves parameters from the specified collection.

Reference: https://api.qdrant.tech/api-reference/collections/get-collection

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}:
    get:
      operationId: get-collection
      summary: Get collection details
      description: Retrieves parameters from the specified collection.
      tags:
        - subpackage_collections
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection to retrieve
          required: true
          schema:
            type: string
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Collections_get_collection_Response_200"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameGetResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: CollectionsCollectionNameGetResponsesContentApplicationJsonSchemaUsage
    CollectionStatus:
      type: string
      enum:
        - green
        - yellow
        - grey
        - red
      description: >-
        Current state of the collection. `Green` - all good. `Yellow` -
        optimization is running, 'Grey' - optimizations are possible but not
        triggered, `Red` - some operations failed and was not recovered
      title: CollectionStatus
    OptimizersStatus0:
      type: string
      enum:
        - ok
      description: Optimizers are reporting as expected
      title: OptimizersStatus0
    OptimizersStatus1:
      type: object
      properties:
        error:
          type: string
      required:
        - error
      description: Something wrong happened with optimizers
      title: OptimizersStatus1
    OptimizersStatus:
      oneOf:
        - $ref: "#/components/schemas/OptimizersStatus0"
        - $ref: "#/components/schemas/OptimizersStatus1"
      description: Current state of the collection
      title: OptimizersStatus
    CollectionWarning:
      type: object
      properties:
        message:
          type: string
          description: Warning message
      required:
        - message
      title: CollectionWarning
    Distance:
      type: string
      enum:
        - Cosine
        - Euclid
        - Dot
        - Manhattan
      description: >-
        Type of internal tags, build from payload Distance function types used
        to compare vectors
      title: Distance
    HnswConfigDiff:
      type: object
      properties:
        m:
          type:
            - integer
            - "null"
          description: >-
            Number of edges per node in the index graph. Larger the value - more
            accurate the search, more space required.
        ef_construct:
          type:
            - integer
            - "null"
          description: >-
            Number of neighbours to consider during the index building. Larger
            the value - more accurate the search, more time required to build
            the index.
        full_scan_threshold:
          type:
            - integer
            - "null"
          description: >-
            Minimal size threshold (in KiloBytes) below which full-scan is
            preferred over HNSW search. This measures the total size of vectors
            being queried against. When the maximum estimated amount of points
            that a condition satisfies is smaller than `full_scan_threshold_kb`,
            the query planner will use full-scan search instead of HNSW index
            traversal for better performance. Note: 1Kb = 1 vector of size 256
        max_indexing_threads:
          type:
            - integer
            - "null"
          description: >-
            Number of parallel threads used for background index building. If 0
            - automatically select from 8 to 16. Best to keep between 8 and 16
            to prevent likelihood of building broken/inefficient HNSW graphs. On
            small CPUs, less threads are used.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store HNSW index on disk. If set to false, the index will be stored
            in RAM. Default: false
        payload_m:
          type:
            - integer
            - "null"
          description: >-
            Custom M param for additional payload-aware HNSW links. If not set,
            default M will be used.
        inline_storage:
          type:
            - boolean
            - "null"
          description: >-
            Store copies of original and quantized vectors within the HNSW index
            file. Default: false. Enabling this option will trade the search
            speed for disk usage by reducing amount of random seeks during the
            search. Requires quantized vectors to be enabled. Multi-vectors are
            not supported.
      title: HnswConfigDiff
    VectorParamsHnswConfig:
      oneOf:
        - $ref: "#/components/schemas/HnswConfigDiff"
        - description: Any type
      description: >-
        Custom params for HNSW index. If none - values from collection
        configuration are used.
      title: VectorParamsHnswConfig
    ScalarType:
      type: string
      enum:
        - int8
      title: ScalarType
    ScalarQuantizationConfig:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/ScalarType"
        quantile:
          type:
            - number
            - "null"
          format: double
          description: >-
            Quantile for quantization. Expected value range in [0.5, 1.0]. If
            not set - use the whole range of values
        always_ram:
          type:
            - boolean
            - "null"
          description: >-
            If true - quantized vectors always will be stored in RAM, ignoring
            the config of main storage
      required:
        - type
      title: ScalarQuantizationConfig
    ScalarQuantization:
      type: object
      properties:
        scalar:
          $ref: "#/components/schemas/ScalarQuantizationConfig"
      required:
        - scalar
      title: ScalarQuantization
    CompressionRatio:
      type: string
      enum:
        - x4
        - x8
        - x16
        - x32
        - x64
      title: CompressionRatio
    ProductQuantizationConfig:
      type: object
      properties:
        compression:
          $ref: "#/components/schemas/CompressionRatio"
        always_ram:
          type:
            - boolean
            - "null"
      required:
        - compression
      title: ProductQuantizationConfig
    ProductQuantization:
      type: object
      properties:
        product:
          $ref: "#/components/schemas/ProductQuantizationConfig"
      required:
        - product
      title: ProductQuantization
    BinaryQuantizationEncoding:
      type: string
      enum:
        - one_bit
        - two_bits
        - one_and_half_bits
      title: BinaryQuantizationEncoding
    BinaryQuantizationConfigEncoding:
      oneOf:
        - $ref: "#/components/schemas/BinaryQuantizationEncoding"
        - description: Any type
      title: BinaryQuantizationConfigEncoding
    BinaryQuantizationQueryEncoding:
      type: string
      enum:
        - default
        - binary
        - scalar4bits
        - scalar8bits
      title: BinaryQuantizationQueryEncoding
    BinaryQuantizationConfigQueryEncoding:
      oneOf:
        - $ref: "#/components/schemas/BinaryQuantizationQueryEncoding"
        - description: Any type
      description: >-
        Asymmetric quantization configuration allows a query to have different
        quantization than stored vectors. It can increase the accuracy of search
        at the cost of performance.
      title: BinaryQuantizationConfigQueryEncoding
    BinaryQuantizationConfig:
      type: object
      properties:
        always_ram:
          type:
            - boolean
            - "null"
        encoding:
          $ref: "#/components/schemas/BinaryQuantizationConfigEncoding"
        query_encoding:
          $ref: "#/components/schemas/BinaryQuantizationConfigQueryEncoding"
          description: >-
            Asymmetric quantization configuration allows a query to have
            different quantization than stored vectors. It can increase the
            accuracy of search at the cost of performance.
      title: BinaryQuantizationConfig
    BinaryQuantization:
      type: object
      properties:
        binary:
          $ref: "#/components/schemas/BinaryQuantizationConfig"
      required:
        - binary
      title: BinaryQuantization
    QuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/ScalarQuantization"
        - $ref: "#/components/schemas/ProductQuantization"
        - $ref: "#/components/schemas/BinaryQuantization"
      title: QuantizationConfig
    VectorParamsQuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/QuantizationConfig"
        - description: Any type
      description: >-
        Custom params for quantization. If none - values from collection
        configuration are used.
      title: VectorParamsQuantizationConfig
    Datatype:
      type: string
      enum:
        - float32
        - uint8
        - float16
      title: Datatype
    VectorParamsDatatype:
      oneOf:
        - $ref: "#/components/schemas/Datatype"
        - description: Any type
      description: >-
        Defines which datatype should be used to represent vectors in the
        storage. Choosing different datatypes allows to optimize memory usage
        and performance vs accuracy.


        - For `float32` datatype - vectors are stored as single-precision
        floating point numbers, 4 bytes. - For `float16` datatype - vectors are
        stored as half-precision floating point numbers, 2 bytes. - For `uint8`
        datatype - vectors are stored as unsigned 8-bit integers, 1 byte. It
        expects vector elements to be in range `[0, 255]`.
      title: VectorParamsDatatype
    MultiVectorComparator:
      type: string
      enum:
        - max_sim
      title: MultiVectorComparator
    MultiVectorConfig:
      type: object
      properties:
        comparator:
          $ref: "#/components/schemas/MultiVectorComparator"
      required:
        - comparator
      title: MultiVectorConfig
    VectorParamsMultivectorConfig:
      oneOf:
        - $ref: "#/components/schemas/MultiVectorConfig"
        - description: Any type
      title: VectorParamsMultivectorConfig
    VectorParams:
      type: object
      properties:
        size:
          type: integer
          format: uint64
          description: Size of a vectors used
        distance:
          $ref: "#/components/schemas/Distance"
        hnsw_config:
          $ref: "#/components/schemas/VectorParamsHnswConfig"
          description: >-
            Custom params for HNSW index. If none - values from collection
            configuration are used.
        quantization_config:
          $ref: "#/components/schemas/VectorParamsQuantizationConfig"
          description: >-
            Custom params for quantization. If none - values from collection
            configuration are used.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            If true, vectors are served from disk, improving RAM usage at the
            cost of latency Default: false
        datatype:
          $ref: "#/components/schemas/VectorParamsDatatype"
          description: >-
            Defines which datatype should be used to represent vectors in the
            storage. Choosing different datatypes allows to optimize memory
            usage and performance vs accuracy.


            - For `float32` datatype - vectors are stored as single-precision
            floating point numbers, 4 bytes. - For `float16` datatype - vectors
            are stored as half-precision floating point numbers, 2 bytes. - For
            `uint8` datatype - vectors are stored as unsigned 8-bit integers, 1
            byte. It expects vector elements to be in range `[0, 255]`.
        multivector_config:
          $ref: "#/components/schemas/VectorParamsMultivectorConfig"
      required:
        - size
        - distance
      description: Params of single vector data storage
      title: VectorParams
    VectorsConfig1:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/VectorParams"
      title: VectorsConfig1
    VectorsConfig:
      oneOf:
        - $ref: "#/components/schemas/VectorParams"
        - $ref: "#/components/schemas/VectorsConfig1"
      description: >-
        Vector params separator for single and multiple vector modes Single
        mode:


        { "size": 128, "distance": "Cosine" }


        or multiple mode:


        { "default": { "size": 128, "distance": "Cosine" } }
      title: VectorsConfig
    ShardingMethod:
      type: string
      enum:
        - auto
        - custom
      title: ShardingMethod
    CollectionParamsShardingMethod:
      oneOf:
        - $ref: "#/components/schemas/ShardingMethod"
        - description: Any type
      description: >-
        Sharding method Default is Auto - points are distributed across all
        available shards Custom - points are distributed across shards according
        to shard key
      title: CollectionParamsShardingMethod
    SparseIndexParamsDatatype:
      oneOf:
        - $ref: "#/components/schemas/Datatype"
        - description: Any type
      description: >-
        Defines which datatype should be used for the index. Choosing different
        datatypes allows to optimize memory usage and performance vs accuracy.


        - For `float32` datatype - vectors are stored as single-precision
        floating point numbers, 4 bytes. - For `float16` datatype - vectors are
        stored as half-precision floating point numbers, 2 bytes. - For `uint8`
        datatype - vectors are quantized to unsigned 8-bit integers, 1 byte.
        Quantization to fit byte range `[0, 255]` happens during indexing
        automatically, so the actual vector data does not need to conform to
        this range.
      title: SparseIndexParamsDatatype
    SparseIndexParams:
      type: object
      properties:
        full_scan_threshold:
          type:
            - integer
            - "null"
          description: >-
            We prefer a full scan search upto (excluding) this number of
            vectors.


            Note: this is number of vectors, not KiloBytes.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store index on disk. If set to false, the index will be stored in
            RAM. Default: false
        datatype:
          $ref: "#/components/schemas/SparseIndexParamsDatatype"
          description: >-
            Defines which datatype should be used for the index. Choosing
            different datatypes allows to optimize memory usage and performance
            vs accuracy.


            - For `float32` datatype - vectors are stored as single-precision
            floating point numbers, 4 bytes. - For `float16` datatype - vectors
            are stored as half-precision floating point numbers, 2 bytes. - For
            `uint8` datatype - vectors are quantized to unsigned 8-bit integers,
            1 byte. Quantization to fit byte range `[0, 255]` happens during
            indexing automatically, so the actual vector data does not need to
            conform to this range.
      description: Configuration for sparse inverted index.
      title: SparseIndexParams
    SparseVectorParamsIndex:
      oneOf:
        - $ref: "#/components/schemas/SparseIndexParams"
        - description: Any type
      description: >-
        Custom params for index. If none - values from collection configuration
        are used.
      title: SparseVectorParamsIndex
    Modifier:
      type: string
      enum:
        - none
        - idf
      description: >-
        If used, include weight modification, which will be applied to sparse
        vectors at query time: None - no modification (default) Idf - inverse
        document frequency, based on statistics of the collection
      title: Modifier
    SparseVectorParamsModifier:
      oneOf:
        - $ref: "#/components/schemas/Modifier"
        - description: Any type
      description: >-
        Configures addition value modifications for sparse vectors. Default:
        none
      title: SparseVectorParamsModifier
    SparseVectorParams:
      type: object
      properties:
        index:
          $ref: "#/components/schemas/SparseVectorParamsIndex"
          description: >-
            Custom params for index. If none - values from collection
            configuration are used.
        modifier:
          $ref: "#/components/schemas/SparseVectorParamsModifier"
          description: >-
            Configures addition value modifications for sparse vectors. Default:
            none
      description: Params of single sparse vector data storage
      title: SparseVectorParams
    CollectionParams:
      type: object
      properties:
        vectors:
          $ref: "#/components/schemas/VectorsConfig"
        shard_number:
          type: integer
          format: uint
          description: Number of shards the collection has
        sharding_method:
          $ref: "#/components/schemas/CollectionParamsShardingMethod"
          description: >-
            Sharding method Default is Auto - points are distributed across all
            available shards Custom - points are distributed across shards
            according to shard key
        replication_factor:
          type: integer
          format: uint
          description: Number of replicas for each shard
        write_consistency_factor:
          type: integer
          format: uint
          description: >-
            Defines how many replicas should apply the operation for us to
            consider it successful. Increasing this number will make the
            collection more resilient to inconsistencies, but will also make it
            fail if not enough replicas are available. Does not have any
            performance impact.
        read_fan_out_factor:
          type:
            - integer
            - "null"
          format: uint
          description: >-
            Defines how many additional replicas should be processing read
            request at the same time. Default value is Auto, which means that
            fan-out will be determined automatically based on the busyness of
            the local replica. Having more than 0 might be useful to smooth
            latency spikes of individual nodes.
        read_fan_out_delay_ms:
          type:
            - integer
            - "null"
          format: uint64
          description: >-
            Define number of milliseconds to wait before attempting to read from
            another replica. This setting can help to reduce latency spikes in
            case of occasional slow replicas. Default is 0, which means delayed
            fan out request is disabled.
        on_disk_payload:
          type: boolean
          default: true
          description: >-
            If true - point's payload will not be stored in memory. It will be
            read from the disk every time it is requested. This setting saves
            RAM by (slightly) increasing the response time. Note: those payload
            values that are involved in filtering and are indexed - remain in
            RAM.


            Default: true
        sparse_vectors:
          type:
            - object
            - "null"
          additionalProperties:
            $ref: "#/components/schemas/SparseVectorParams"
          description: Configuration of the sparse vector storage
      title: CollectionParams
    HnswConfig:
      type: object
      properties:
        m:
          type: integer
          description: >-
            Number of edges per node in the index graph. Larger the value - more
            accurate the search, more space required.
        ef_construct:
          type: integer
          description: >-
            Number of neighbours to consider during the index building. Larger
            the value - more accurate the search, more time required to build
            index.
        full_scan_threshold:
          type: integer
          description: >-
            Minimal size threshold (in KiloBytes) below which full-scan is
            preferred over HNSW search. This measures the total size of vectors
            being queried against. When the maximum estimated amount of points
            that a condition satisfies is smaller than `full_scan_threshold_kb`,
            the query planner will use full-scan search instead of HNSW index
            traversal for better performance. Note: 1Kb = 1 vector of size 256
        max_indexing_threads:
          type: integer
          default: 0
          description: >-
            Number of parallel threads used for background index building. If 0
            - automatically select from 8 to 16. Best to keep between 8 and 16
            to prevent likelihood of slow building or broken/inefficient HNSW
            graphs. On small CPUs, less threads are used.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store HNSW index on disk. If set to false, index will be stored in
            RAM. Default: false
        payload_m:
          type:
            - integer
            - "null"
          description: >-
            Custom M param for hnsw graph built for payload index. If not set,
            default M will be used.
        inline_storage:
          type:
            - boolean
            - "null"
          description: >-
            Store copies of original and quantized vectors within the HNSW index
            file. Default: false. Enabling this option will trade the search
            speed for disk usage by reducing amount of random seeks during the
            search. Requires quantized vectors to be enabled. Multi-vectors are
            not supported.
      required:
        - m
        - ef_construct
        - full_scan_threshold
      description: Config of HNSW index
      title: HnswConfig
    OptimizersConfig:
      type: object
      properties:
        deleted_threshold:
          type: number
          format: double
          description: >-
            The minimal fraction of deleted vectors in a segment, required to
            perform segment optimization
        vacuum_min_vector_number:
          type: integer
          description: >-
            The minimal number of vectors in a segment, required to perform
            segment optimization
        default_segment_number:
          type: integer
          description: >-
            Target amount of segments optimizer will try to keep. Real amount of
            segments may vary depending on multiple parameters: - Amount of
            stored points - Current write RPS


            It is recommended to select default number of segments as a factor
            of the number of search threads, so that each segment would be
            handled evenly by one of the threads. If `default_segment_number =
            0`, will be automatically selected by the number of available CPUs.
        max_segment_size:
          type:
            - integer
            - "null"
          description: >-
            Do not create segments larger this size (in kilobytes). Large
            segments might require disproportionately long indexation times,
            therefore it makes sense to limit the size of segments.


            If indexing speed is more important - make this parameter lower. If
            search speed is more important - make this parameter higher. Note:
            1Kb = 1 vector of size 256 If not set, will be automatically
            selected considering the number of available CPUs.
        memmap_threshold:
          type:
            - integer
            - "null"
          description: >-
            Maximum size (in kilobytes) of vectors to store in-memory per
            segment. Segments larger than this threshold will be stored as
            read-only memmapped file.


            Memmap storage is disabled by default, to enable it, set this
            threshold to a reasonable value.


            To disable memmap storage, set this to `0`. Internally it will use
            the largest threshold possible.


            Note: 1Kb = 1 vector of size 256
        indexing_threshold:
          type:
            - integer
            - "null"
          description: >-
            Maximum size (in kilobytes) of vectors allowed for plain index,
            exceeding this threshold will enable vector indexing


            Default value is 10,000, based on experiments and observations.


            To disable vector indexing, set to `0`.


            Note: 1kB = 1 vector of size 256.
        flush_interval_sec:
          type: integer
          format: uint64
          description: Minimum interval between forced flushes.
        max_optimization_threads:
          type:
            - integer
            - "null"
          description: >-
            Max number of threads (jobs) for running optimizations per shard.
            Note: each optimization job will also use `max_indexing_threads`
            threads by itself for index building. If null - have no limit and
            choose dynamically to saturate CPU. If 0 - no optimization threads,
            optimizations will be disabled.
        prevent_unoptimized:
          type:
            - boolean
            - "null"
          description: >-
            If this option is set, service will try to prevent creation of large
            unoptimized segments. When enabled, updates may be blocked at
            request level if there are unoptimized segments larger than indexing
            threshold. Updates will be resumed when optimization is completed
            and segments are optimized below the threshold. Using this option
            may lead to increased delay between submitting an update and its
            application. Default is disabled.
      required:
        - deleted_threshold
        - vacuum_min_vector_number
        - default_segment_number
        - flush_interval_sec
      title: OptimizersConfig
    WalConfig:
      type: object
      properties:
        wal_capacity_mb:
          type: integer
          description: Size of a single WAL segment in MB
        wal_segments_ahead:
          type: integer
          description: Number of WAL segments to create ahead of actually used ones
        wal_retain_closed:
          type: integer
          default: 1
          description: Number of closed WAL segments to keep
      required:
        - wal_capacity_mb
        - wal_segments_ahead
      title: WalConfig
    CollectionConfigWalConfig:
      oneOf:
        - $ref: "#/components/schemas/WalConfig"
        - description: Any type
      title: CollectionConfigWalConfig
    CollectionConfigQuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/QuantizationConfig"
        - description: Any type
      title: CollectionConfigQuantizationConfig
    StrictModeMultivectorOutput:
      type: object
      properties:
        max_vectors:
          type:
            - integer
            - "null"
          description: Max number of vectors in a multivector
      title: StrictModeMultivectorOutput
    StrictModeMultivectorConfigOutput:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/StrictModeMultivectorOutput"
      title: StrictModeMultivectorConfigOutput
    StrictModeConfigOutputMultivectorConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeMultivectorConfigOutput"
        - description: Any type
      description: Multivector configuration
      title: StrictModeConfigOutputMultivectorConfig
    StrictModeSparseOutput:
      type: object
      properties:
        max_length:
          type:
            - integer
            - "null"
          description: Max length of sparse vector
      title: StrictModeSparseOutput
    StrictModeSparseConfigOutput:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/StrictModeSparseOutput"
      title: StrictModeSparseConfigOutput
    StrictModeConfigOutputSparseConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeSparseConfigOutput"
        - description: Any type
      description: Sparse vector configuration
      title: StrictModeConfigOutputSparseConfig
    StrictModeConfigOutput:
      type: object
      properties:
        enabled:
          type:
            - boolean
            - "null"
          description: Whether strict mode is enabled for a collection or not.
        max_query_limit:
          type:
            - integer
            - "null"
          description: >-
            Max allowed `limit` parameter for all APIs that don't have their own
            max limit.
        max_timeout:
          type:
            - integer
            - "null"
          description: Max allowed `timeout` parameter.
        unindexed_filtering_retrieve:
          type:
            - boolean
            - "null"
          description: >-
            Allow usage of unindexed fields in retrieval based (e.g. search)
            filters.
        unindexed_filtering_update:
          type:
            - boolean
            - "null"
          description: >-
            Allow usage of unindexed fields in filtered updates (e.g. delete by
            payload).
        search_max_hnsw_ef:
          type:
            - integer
            - "null"
          description: Max HNSW value allowed in search parameters.
        search_allow_exact:
          type:
            - boolean
            - "null"
          description: Whether exact search is allowed or not.
        search_max_oversampling:
          type:
            - number
            - "null"
          format: double
          description: Max oversampling value allowed in search.
        upsert_max_batchsize:
          type:
            - integer
            - "null"
          description: Max batchsize when upserting
        max_collection_vector_size_bytes:
          type:
            - integer
            - "null"
          description: >-
            Max size of a collections vector storage in bytes, ignoring
            replicas.
        read_rate_limit:
          type:
            - integer
            - "null"
          description: Max number of read operations per minute per replica
        write_rate_limit:
          type:
            - integer
            - "null"
          description: Max number of write operations per minute per replica
        max_collection_payload_size_bytes:
          type:
            - integer
            - "null"
          description: Max size of a collections payload storage in bytes
        max_points_count:
          type:
            - integer
            - "null"
          description: Max number of points estimated in a collection
        filter_max_conditions:
          type:
            - integer
            - "null"
          description: Max conditions a filter can have.
        condition_max_size:
          type:
            - integer
            - "null"
          description: Max size of a condition, eg. items in `MatchAny`.
        multivector_config:
          $ref: "#/components/schemas/StrictModeConfigOutputMultivectorConfig"
          description: Multivector configuration
        sparse_config:
          $ref: "#/components/schemas/StrictModeConfigOutputSparseConfig"
          description: Sparse vector configuration
        max_payload_index_count:
          type:
            - integer
            - "null"
          description: Max number of payload indexes in a collection
      title: StrictModeConfigOutput
    CollectionConfigStrictModeConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeConfigOutput"
        - description: Any type
      title: CollectionConfigStrictModeConfig
    Payload:
      type: object
      additionalProperties:
        description: Any type
      title: Payload
    CollectionConfigMetadata:
      oneOf:
        - $ref: "#/components/schemas/Payload"
        - description: Any type
      description: >-
        Arbitrary JSON metadata for the collection This can be used to store
        application-specific information such as creation time, migration data,
        inference model info, etc.
      title: CollectionConfigMetadata
    CollectionConfig:
      type: object
      properties:
        params:
          $ref: "#/components/schemas/CollectionParams"
        hnsw_config:
          $ref: "#/components/schemas/HnswConfig"
        optimizer_config:
          $ref: "#/components/schemas/OptimizersConfig"
        wal_config:
          $ref: "#/components/schemas/CollectionConfigWalConfig"
        quantization_config:
          $ref: "#/components/schemas/CollectionConfigQuantizationConfig"
        strict_mode_config:
          $ref: "#/components/schemas/CollectionConfigStrictModeConfig"
        metadata:
          $ref: "#/components/schemas/CollectionConfigMetadata"
          description: >-
            Arbitrary JSON metadata for the collection This can be used to store
            application-specific information such as creation time, migration
            data, inference model info, etc.
      required:
        - params
        - hnsw_config
        - optimizer_config
      description: Information about the collection configuration
      title: CollectionConfig
    PayloadSchemaType:
      type: string
      enum:
        - keyword
        - integer
        - float
        - geo
        - text
        - bool
        - datetime
        - uuid
      description: All possible names of payload types
      title: PayloadSchemaType
    KeywordIndexType:
      type: string
      enum:
        - keyword
      title: KeywordIndexType
    KeywordIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/KeywordIndexType"
        is_tenant:
          type:
            - boolean
            - "null"
          description: "If true - used for tenant optimization. Default: false."
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: KeywordIndexParams
    IntegerIndexType:
      type: string
      enum:
        - integer
      title: IntegerIndexType
    IntegerIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/IntegerIndexType"
        lookup:
          type:
            - boolean
            - "null"
          description: If true - support direct lookups. Default is true.
        range:
          type:
            - boolean
            - "null"
          description: If true - support ranges filters. Default is true.
        is_principal:
          type:
            - boolean
            - "null"
          description: >-
            If true - use this key to organize storage of the collection data.
            This option assumes that this key will be used in majority of
            filtered requests. Default is false.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false. Default is false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: IntegerIndexParams
    FloatIndexType:
      type: string
      enum:
        - float
      title: FloatIndexType
    FloatIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/FloatIndexType"
        is_principal:
          type:
            - boolean
            - "null"
          description: >-
            If true - use this key to organize storage of the collection data.
            This option assumes that this key will be used in majority of
            filtered requests.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: FloatIndexParams
    GeoIndexType:
      type: string
      enum:
        - geo
      title: GeoIndexType
    GeoIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/GeoIndexType"
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: GeoIndexParams
    TextIndexType:
      type: string
      enum:
        - text
      title: TextIndexType
    TokenizerType:
      type: string
      enum:
        - prefix
        - whitespace
        - word
        - multilingual
      title: TokenizerType
    Language:
      type: string
      enum:
        - arabic
        - azerbaijani
        - basque
        - bengali
        - catalan
        - chinese
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hebrew
        - hinglish
        - hungarian
        - indonesian
        - italian
        - japanese
        - kazakh
        - nepali
        - norwegian
        - portuguese
        - romanian
        - russian
        - slovene
        - spanish
        - swedish
        - tajik
        - turkish
      title: Language
    StopwordsSet:
      type: object
      properties:
        languages:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/Language"
          description: >-
            Set of languages to use for stopwords. Multiple pre-defined lists of
            stopwords can be combined.
        custom:
          type:
            - array
            - "null"
          items:
            type: string
          description: Custom stopwords set. Will be merged with the languages set.
      title: StopwordsSet
    StopwordsInterface:
      oneOf:
        - $ref: "#/components/schemas/Language"
        - $ref: "#/components/schemas/StopwordsSet"
      title: StopwordsInterface
    TextIndexParamsStopwords:
      oneOf:
        - $ref: "#/components/schemas/StopwordsInterface"
        - description: Any type
      description: >-
        Ignore this set of tokens. Can select from predefined languages and/or
        provide a custom set.
      title: TextIndexParamsStopwords
    Snowball:
      type: string
      enum:
        - snowball
      title: Snowball
    SnowballLanguage:
      type: string
      enum:
        - arabic
        - armenian
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hungarian
        - italian
        - norwegian
        - portuguese
        - romanian
        - russian
        - spanish
        - swedish
        - tamil
        - turkish
      description: Languages supported by snowball stemmer.
      title: SnowballLanguage
    SnowballParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/Snowball"
        language:
          $ref: "#/components/schemas/SnowballLanguage"
      required:
        - type
        - language
      title: SnowballParams
    StemmingAlgorithm:
      oneOf:
        - $ref: "#/components/schemas/SnowballParams"
      description: Different stemming algorithms with their configs.
      title: StemmingAlgorithm
    TextIndexParamsStemmer:
      oneOf:
        - $ref: "#/components/schemas/StemmingAlgorithm"
        - description: Any type
      description: "Algorithm for stemming. Default: disabled."
      title: TextIndexParamsStemmer
    TextIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/TextIndexType"
        tokenizer:
          $ref: "#/components/schemas/TokenizerType"
        min_token_len:
          type:
            - integer
            - "null"
          description: Minimum characters to be tokenized.
        max_token_len:
          type:
            - integer
            - "null"
          description: Maximum characters to be tokenized.
        lowercase:
          type:
            - boolean
            - "null"
          description: "If true, lowercase all tokens. Default: true."
        ascii_folding:
          type:
            - boolean
            - "null"
          description: >-
            If true, normalize tokens by folding accented characters to ASCII
            (e.g., "ação" -> "acao"). Default: false.
        phrase_matching:
          type:
            - boolean
            - "null"
          description: "If true, support phrase matching. Default: false."
        stopwords:
          $ref: "#/components/schemas/TextIndexParamsStopwords"
          description: >-
            Ignore this set of tokens. Can select from predefined languages
            and/or provide a custom set.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        stemmer:
          $ref: "#/components/schemas/TextIndexParamsStemmer"
          description: "Algorithm for stemming. Default: disabled."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: TextIndexParams
    BoolIndexType:
      type: string
      enum:
        - bool
      title: BoolIndexType
    BoolIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/BoolIndexType"
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: BoolIndexParams
    DatetimeIndexType:
      type: string
      enum:
        - datetime
      title: DatetimeIndexType
    DatetimeIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/DatetimeIndexType"
        is_principal:
          type:
            - boolean
            - "null"
          description: >-
            If true - use this key to organize storage of the collection data.
            This option assumes that this key will be used in majority of
            filtered requests.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: DatetimeIndexParams
    UuidIndexType:
      type: string
      enum:
        - uuid
      title: UuidIndexType
    UuidIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/UuidIndexType"
        is_tenant:
          type:
            - boolean
            - "null"
          description: If true - used for tenant optimization.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: UuidIndexParams
    PayloadSchemaParams:
      oneOf:
        - $ref: "#/components/schemas/KeywordIndexParams"
        - $ref: "#/components/schemas/IntegerIndexParams"
        - $ref: "#/components/schemas/FloatIndexParams"
        - $ref: "#/components/schemas/GeoIndexParams"
        - $ref: "#/components/schemas/TextIndexParams"
        - $ref: "#/components/schemas/BoolIndexParams"
        - $ref: "#/components/schemas/DatetimeIndexParams"
        - $ref: "#/components/schemas/UuidIndexParams"
      description: Payload type with parameters
      title: PayloadSchemaParams
    PayloadIndexInfoParams:
      oneOf:
        - $ref: "#/components/schemas/PayloadSchemaParams"
        - description: Any type
      title: PayloadIndexInfoParams
    PayloadIndexInfo:
      type: object
      properties:
        data_type:
          $ref: "#/components/schemas/PayloadSchemaType"
        params:
          $ref: "#/components/schemas/PayloadIndexInfoParams"
        points:
          type: integer
          description: Number of points indexed with this index
      required:
        - data_type
        - points
      description: Display payload field type & index information
      title: PayloadIndexInfo
    UpdateQueueInfo:
      type: object
      properties:
        length:
          type: integer
          description: Number of elements in the queue
      required:
        - length
      title: UpdateQueueInfo
    CollectionInfoUpdateQueue:
      oneOf:
        - $ref: "#/components/schemas/UpdateQueueInfo"
        - description: Any type
      description: Update queue info
      title: CollectionInfoUpdateQueue
    CollectionInfo:
      type: object
      properties:
        status:
          $ref: "#/components/schemas/CollectionStatus"
        optimizer_status:
          $ref: "#/components/schemas/OptimizersStatus"
        warnings:
          type: array
          items:
            $ref: "#/components/schemas/CollectionWarning"
          description: Warnings related to the collection
        indexed_vectors_count:
          type:
            - integer
            - "null"
          description: >-
            Approximate number of indexed vectors in the collection. Indexed
            vectors in large segments are faster to query, as it is stored in a
            specialized vector index.
        points_count:
          type:
            - integer
            - "null"
          description: >-
            Approximate number of points (vectors + payloads) in collection.
            Each point could be accessed by unique id.
        segments_count:
          type: integer
          description: >-
            Number of segments in collection. Each segment has independent
            vector as payload indexes
        config:
          $ref: "#/components/schemas/CollectionConfig"
        payload_schema:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/PayloadIndexInfo"
          description: Types of stored payload
        update_queue:
          $ref: "#/components/schemas/CollectionInfoUpdateQueue"
          description: Update queue info
      required:
        - status
        - optimizer_status
        - segments_count
        - config
        - payload_schema
      description: Current statistics and configuration of the collection
      title: CollectionInfo
    Collections_get_collection_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameGetResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/CollectionInfo"
      title: Collections_get_collection_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.get_collection("{collection_name}")

```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.collection_info("{collection_name}").await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.getCollectionInfoAsync("{collection_name}").get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.getCollection("{collection_name}");
```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func getCollection() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	info, err := client.GetCollectionInfo(context.Background(), "{collection_name}")
	if err != nil {
		panic(err)
	}
	fmt.Println("Collection info: ", info)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.GetCollectionInfoAsync("{collection_name}");

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Get.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('GET', 'http://localhost:6333/collections/collection_name', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "GET"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Create a collection

PUT http://localhost:6333/collections/{collection_name}
Content-Type: application/json

Creates a new collection with the given parameters.

Reference: https://api.qdrant.tech/api-reference/collections/create-collection

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}:
    put:
      operationId: create-collection
      summary: Create a collection
      description: Creates a new collection with the given parameters.
      tags:
        - subpackage_collections
      parameters:
        - name: collection_name
          in: path
          description: Name of the new collection
          required: true
          schema:
            type: string
        - name: timeout
          in: query
          description: |
            Wait for operation commit timeout in seconds.
            If timeout is reached - request will return with service error.
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: >-
                  #/components/schemas/Collections_create_collection_Response_200
      requestBody:
        description: Parameters of a new collection
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/CreateCollection"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    Distance:
      type: string
      enum:
        - Cosine
        - Euclid
        - Dot
        - Manhattan
      description: >-
        Type of internal tags, build from payload Distance function types used
        to compare vectors
      title: Distance
    HnswConfigDiff:
      type: object
      properties:
        m:
          type:
            - integer
            - "null"
          description: >-
            Number of edges per node in the index graph. Larger the value - more
            accurate the search, more space required.
        ef_construct:
          type:
            - integer
            - "null"
          description: >-
            Number of neighbours to consider during the index building. Larger
            the value - more accurate the search, more time required to build
            the index.
        full_scan_threshold:
          type:
            - integer
            - "null"
          description: >-
            Minimal size threshold (in KiloBytes) below which full-scan is
            preferred over HNSW search. This measures the total size of vectors
            being queried against. When the maximum estimated amount of points
            that a condition satisfies is smaller than `full_scan_threshold_kb`,
            the query planner will use full-scan search instead of HNSW index
            traversal for better performance. Note: 1Kb = 1 vector of size 256
        max_indexing_threads:
          type:
            - integer
            - "null"
          description: >-
            Number of parallel threads used for background index building. If 0
            - automatically select from 8 to 16. Best to keep between 8 and 16
            to prevent likelihood of building broken/inefficient HNSW graphs. On
            small CPUs, less threads are used.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store HNSW index on disk. If set to false, the index will be stored
            in RAM. Default: false
        payload_m:
          type:
            - integer
            - "null"
          description: >-
            Custom M param for additional payload-aware HNSW links. If not set,
            default M will be used.
        inline_storage:
          type:
            - boolean
            - "null"
          description: >-
            Store copies of original and quantized vectors within the HNSW index
            file. Default: false. Enabling this option will trade the search
            speed for disk usage by reducing amount of random seeks during the
            search. Requires quantized vectors to be enabled. Multi-vectors are
            not supported.
      title: HnswConfigDiff
    VectorParamsHnswConfig:
      oneOf:
        - $ref: "#/components/schemas/HnswConfigDiff"
        - description: Any type
      description: >-
        Custom params for HNSW index. If none - values from collection
        configuration are used.
      title: VectorParamsHnswConfig
    ScalarType:
      type: string
      enum:
        - int8
      title: ScalarType
    ScalarQuantizationConfig:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/ScalarType"
        quantile:
          type:
            - number
            - "null"
          format: double
          description: >-
            Quantile for quantization. Expected value range in [0.5, 1.0]. If
            not set - use the whole range of values
        always_ram:
          type:
            - boolean
            - "null"
          description: >-
            If true - quantized vectors always will be stored in RAM, ignoring
            the config of main storage
      required:
        - type
      title: ScalarQuantizationConfig
    ScalarQuantization:
      type: object
      properties:
        scalar:
          $ref: "#/components/schemas/ScalarQuantizationConfig"
      required:
        - scalar
      title: ScalarQuantization
    CompressionRatio:
      type: string
      enum:
        - x4
        - x8
        - x16
        - x32
        - x64
      title: CompressionRatio
    ProductQuantizationConfig:
      type: object
      properties:
        compression:
          $ref: "#/components/schemas/CompressionRatio"
        always_ram:
          type:
            - boolean
            - "null"
      required:
        - compression
      title: ProductQuantizationConfig
    ProductQuantization:
      type: object
      properties:
        product:
          $ref: "#/components/schemas/ProductQuantizationConfig"
      required:
        - product
      title: ProductQuantization
    BinaryQuantizationEncoding:
      type: string
      enum:
        - one_bit
        - two_bits
        - one_and_half_bits
      title: BinaryQuantizationEncoding
    BinaryQuantizationConfigEncoding:
      oneOf:
        - $ref: "#/components/schemas/BinaryQuantizationEncoding"
        - description: Any type
      title: BinaryQuantizationConfigEncoding
    BinaryQuantizationQueryEncoding:
      type: string
      enum:
        - default
        - binary
        - scalar4bits
        - scalar8bits
      title: BinaryQuantizationQueryEncoding
    BinaryQuantizationConfigQueryEncoding:
      oneOf:
        - $ref: "#/components/schemas/BinaryQuantizationQueryEncoding"
        - description: Any type
      description: >-
        Asymmetric quantization configuration allows a query to have different
        quantization than stored vectors. It can increase the accuracy of search
        at the cost of performance.
      title: BinaryQuantizationConfigQueryEncoding
    BinaryQuantizationConfig:
      type: object
      properties:
        always_ram:
          type:
            - boolean
            - "null"
        encoding:
          $ref: "#/components/schemas/BinaryQuantizationConfigEncoding"
        query_encoding:
          $ref: "#/components/schemas/BinaryQuantizationConfigQueryEncoding"
          description: >-
            Asymmetric quantization configuration allows a query to have
            different quantization than stored vectors. It can increase the
            accuracy of search at the cost of performance.
      title: BinaryQuantizationConfig
    BinaryQuantization:
      type: object
      properties:
        binary:
          $ref: "#/components/schemas/BinaryQuantizationConfig"
      required:
        - binary
      title: BinaryQuantization
    QuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/ScalarQuantization"
        - $ref: "#/components/schemas/ProductQuantization"
        - $ref: "#/components/schemas/BinaryQuantization"
      title: QuantizationConfig
    VectorParamsQuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/QuantizationConfig"
        - description: Any type
      description: >-
        Custom params for quantization. If none - values from collection
        configuration are used.
      title: VectorParamsQuantizationConfig
    Datatype:
      type: string
      enum:
        - float32
        - uint8
        - float16
      title: Datatype
    VectorParamsDatatype:
      oneOf:
        - $ref: "#/components/schemas/Datatype"
        - description: Any type
      description: >-
        Defines which datatype should be used to represent vectors in the
        storage. Choosing different datatypes allows to optimize memory usage
        and performance vs accuracy.


        - For `float32` datatype - vectors are stored as single-precision
        floating point numbers, 4 bytes. - For `float16` datatype - vectors are
        stored as half-precision floating point numbers, 2 bytes. - For `uint8`
        datatype - vectors are stored as unsigned 8-bit integers, 1 byte. It
        expects vector elements to be in range `[0, 255]`.
      title: VectorParamsDatatype
    MultiVectorComparator:
      type: string
      enum:
        - max_sim
      title: MultiVectorComparator
    MultiVectorConfig:
      type: object
      properties:
        comparator:
          $ref: "#/components/schemas/MultiVectorComparator"
      required:
        - comparator
      title: MultiVectorConfig
    VectorParamsMultivectorConfig:
      oneOf:
        - $ref: "#/components/schemas/MultiVectorConfig"
        - description: Any type
      title: VectorParamsMultivectorConfig
    VectorParams:
      type: object
      properties:
        size:
          type: integer
          format: uint64
          description: Size of a vectors used
        distance:
          $ref: "#/components/schemas/Distance"
        hnsw_config:
          $ref: "#/components/schemas/VectorParamsHnswConfig"
          description: >-
            Custom params for HNSW index. If none - values from collection
            configuration are used.
        quantization_config:
          $ref: "#/components/schemas/VectorParamsQuantizationConfig"
          description: >-
            Custom params for quantization. If none - values from collection
            configuration are used.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            If true, vectors are served from disk, improving RAM usage at the
            cost of latency Default: false
        datatype:
          $ref: "#/components/schemas/VectorParamsDatatype"
          description: >-
            Defines which datatype should be used to represent vectors in the
            storage. Choosing different datatypes allows to optimize memory
            usage and performance vs accuracy.


            - For `float32` datatype - vectors are stored as single-precision
            floating point numbers, 4 bytes. - For `float16` datatype - vectors
            are stored as half-precision floating point numbers, 2 bytes. - For
            `uint8` datatype - vectors are stored as unsigned 8-bit integers, 1
            byte. It expects vector elements to be in range `[0, 255]`.
        multivector_config:
          $ref: "#/components/schemas/VectorParamsMultivectorConfig"
      required:
        - size
        - distance
      description: Params of single vector data storage
      title: VectorParams
    VectorsConfig1:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/VectorParams"
      title: VectorsConfig1
    VectorsConfig:
      oneOf:
        - $ref: "#/components/schemas/VectorParams"
        - $ref: "#/components/schemas/VectorsConfig1"
      description: >-
        Vector params separator for single and multiple vector modes Single
        mode:


        { "size": 128, "distance": "Cosine" }


        or multiple mode:


        { "default": { "size": 128, "distance": "Cosine" } }
      title: VectorsConfig
    ShardingMethod:
      type: string
      enum:
        - auto
        - custom
      title: ShardingMethod
    CreateCollectionShardingMethod:
      oneOf:
        - $ref: "#/components/schemas/ShardingMethod"
        - description: Any type
      description: >-
        Sharding method Default is Auto - points are distributed across all
        available shards Custom - points are distributed across shards according
        to shard key
      title: CreateCollectionShardingMethod
    CreateCollectionHnswConfig:
      oneOf:
        - $ref: "#/components/schemas/HnswConfigDiff"
        - description: Any type
      description: >-
        Custom params for HNSW index. If none - values from service
        configuration file are used.
      title: CreateCollectionHnswConfig
    WalConfigDiff:
      type: object
      properties:
        wal_capacity_mb:
          type:
            - integer
            - "null"
          description: Size of a single WAL segment in MB
        wal_segments_ahead:
          type:
            - integer
            - "null"
          description: Number of WAL segments to create ahead of actually used ones
        wal_retain_closed:
          type:
            - integer
            - "null"
          description: Number of closed WAL segments to retain
      title: WalConfigDiff
    CreateCollectionWalConfig:
      oneOf:
        - $ref: "#/components/schemas/WalConfigDiff"
        - description: Any type
      description: >-
        Custom params for WAL. If none - values from service configuration file
        are used.
      title: CreateCollectionWalConfig
    MaxOptimizationThreadsSetting:
      type: string
      enum:
        - auto
      title: MaxOptimizationThreadsSetting
    MaxOptimizationThreads:
      oneOf:
        - $ref: "#/components/schemas/MaxOptimizationThreadsSetting"
        - type: integer
      title: MaxOptimizationThreads
    OptimizersConfigDiffMaxOptimizationThreads:
      oneOf:
        - $ref: "#/components/schemas/MaxOptimizationThreads"
        - description: Any type
      description: >-
        Max number of threads (jobs) for running optimizations per shard. Note:
        each optimization job will also use `max_indexing_threads` threads by
        itself for index building. If "auto" - have no limit and choose
        dynamically to saturate CPU. If 0 - no optimization threads,
        optimizations will be disabled.
      title: OptimizersConfigDiffMaxOptimizationThreads
    OptimizersConfigDiff:
      type: object
      properties:
        deleted_threshold:
          type:
            - number
            - "null"
          format: double
          description: >-
            The minimal fraction of deleted vectors in a segment, required to
            perform segment optimization
        vacuum_min_vector_number:
          type:
            - integer
            - "null"
          description: >-
            The minimal number of vectors in a segment, required to perform
            segment optimization
        default_segment_number:
          type:
            - integer
            - "null"
          description: >-
            Target amount of segments optimizer will try to keep. Real amount of
            segments may vary depending on multiple parameters: - Amount of
            stored points - Current write RPS


            It is recommended to select default number of segments as a factor
            of the number of search threads, so that each segment would be
            handled evenly by one of the threads If `default_segment_number =
            0`, will be automatically selected by the number of available CPUs
        max_segment_size:
          type:
            - integer
            - "null"
          description: >-
            Do not create segments larger this size (in kilobytes). Large
            segments might require disproportionately long indexation times,
            therefore it makes sense to limit the size of segments.


            If indexation speed have more priority for your - make this
            parameter lower. If search speed is more important - make this
            parameter higher. Note: 1Kb = 1 vector of size 256
        memmap_threshold:
          type:
            - integer
            - "null"
          description: >-
            Maximum size (in kilobytes) of vectors to store in-memory per
            segment. Segments larger than this threshold will be stored as
            read-only memmapped file.


            Memmap storage is disabled by default, to enable it, set this
            threshold to a reasonable value.


            To disable memmap storage, set this to `0`.


            Note: 1Kb = 1 vector of size 256


            Deprecated since Qdrant 1.15.0
        indexing_threshold:
          type:
            - integer
            - "null"
          description: >-
            Maximum size (in kilobytes) of vectors allowed for plain index,
            exceeding this threshold will enable vector indexing


            Default value is 20,000, based on
            <https://github.com/google-research/google-research/blob/master/scann/docs/algorithms.md>.


            To disable vector indexing, set to `0`.


            Note: 1kB = 1 vector of size 256.
        flush_interval_sec:
          type:
            - integer
            - "null"
          format: uint64
          description: Minimum interval between forced flushes.
        max_optimization_threads:
          $ref: "#/components/schemas/OptimizersConfigDiffMaxOptimizationThreads"
          description: >-
            Max number of threads (jobs) for running optimizations per shard.
            Note: each optimization job will also use `max_indexing_threads`
            threads by itself for index building. If "auto" - have no limit and
            choose dynamically to saturate CPU. If 0 - no optimization threads,
            optimizations will be disabled.
        prevent_unoptimized:
          type:
            - boolean
            - "null"
          description: >-
            If this option is set, service will try to prevent creation of large
            unoptimized segments. When enabled, updates may be blocked at
            request level if there are unoptimized segments larger than indexing
            threshold. Updates will be resumed when optimization is completed
            and segments are optimized below the threshold. Using this option
            may lead to increased delay between submitting an update and its
            application. Default is disabled.
      title: OptimizersConfigDiff
    CreateCollectionOptimizersConfig:
      oneOf:
        - $ref: "#/components/schemas/OptimizersConfigDiff"
        - description: Any type
      description: >-
        Custom params for Optimizers.  If none - values from service
        configuration file are used.
      title: CreateCollectionOptimizersConfig
    CreateCollectionQuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/QuantizationConfig"
        - description: Any type
      description: Quantization parameters. If none - quantization is disabled.
      title: CreateCollectionQuantizationConfig
    SparseIndexParamsDatatype:
      oneOf:
        - $ref: "#/components/schemas/Datatype"
        - description: Any type
      description: >-
        Defines which datatype should be used for the index. Choosing different
        datatypes allows to optimize memory usage and performance vs accuracy.


        - For `float32` datatype - vectors are stored as single-precision
        floating point numbers, 4 bytes. - For `float16` datatype - vectors are
        stored as half-precision floating point numbers, 2 bytes. - For `uint8`
        datatype - vectors are quantized to unsigned 8-bit integers, 1 byte.
        Quantization to fit byte range `[0, 255]` happens during indexing
        automatically, so the actual vector data does not need to conform to
        this range.
      title: SparseIndexParamsDatatype
    SparseIndexParams:
      type: object
      properties:
        full_scan_threshold:
          type:
            - integer
            - "null"
          description: >-
            We prefer a full scan search upto (excluding) this number of
            vectors.


            Note: this is number of vectors, not KiloBytes.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store index on disk. If set to false, the index will be stored in
            RAM. Default: false
        datatype:
          $ref: "#/components/schemas/SparseIndexParamsDatatype"
          description: >-
            Defines which datatype should be used for the index. Choosing
            different datatypes allows to optimize memory usage and performance
            vs accuracy.


            - For `float32` datatype - vectors are stored as single-precision
            floating point numbers, 4 bytes. - For `float16` datatype - vectors
            are stored as half-precision floating point numbers, 2 bytes. - For
            `uint8` datatype - vectors are quantized to unsigned 8-bit integers,
            1 byte. Quantization to fit byte range `[0, 255]` happens during
            indexing automatically, so the actual vector data does not need to
            conform to this range.
      description: Configuration for sparse inverted index.
      title: SparseIndexParams
    SparseVectorParamsIndex:
      oneOf:
        - $ref: "#/components/schemas/SparseIndexParams"
        - description: Any type
      description: >-
        Custom params for index. If none - values from collection configuration
        are used.
      title: SparseVectorParamsIndex
    Modifier:
      type: string
      enum:
        - none
        - idf
      description: >-
        If used, include weight modification, which will be applied to sparse
        vectors at query time: None - no modification (default) Idf - inverse
        document frequency, based on statistics of the collection
      title: Modifier
    SparseVectorParamsModifier:
      oneOf:
        - $ref: "#/components/schemas/Modifier"
        - description: Any type
      description: >-
        Configures addition value modifications for sparse vectors. Default:
        none
      title: SparseVectorParamsModifier
    SparseVectorParams:
      type: object
      properties:
        index:
          $ref: "#/components/schemas/SparseVectorParamsIndex"
          description: >-
            Custom params for index. If none - values from collection
            configuration are used.
        modifier:
          $ref: "#/components/schemas/SparseVectorParamsModifier"
          description: >-
            Configures addition value modifications for sparse vectors. Default:
            none
      description: Params of single sparse vector data storage
      title: SparseVectorParams
    StrictModeMultivector:
      type: object
      properties:
        max_vectors:
          type:
            - integer
            - "null"
          description: Max number of vectors in a multivector
      title: StrictModeMultivector
    StrictModeMultivectorConfig:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/StrictModeMultivector"
      title: StrictModeMultivectorConfig
    StrictModeConfigMultivectorConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeMultivectorConfig"
        - description: Any type
      description: Multivector strict mode configuration
      title: StrictModeConfigMultivectorConfig
    StrictModeSparse:
      type: object
      properties:
        max_length:
          type:
            - integer
            - "null"
          description: Max length of sparse vector
      title: StrictModeSparse
    StrictModeSparseConfig:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/StrictModeSparse"
      title: StrictModeSparseConfig
    StrictModeConfigSparseConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeSparseConfig"
        - description: Any type
      description: Sparse vector strict mode configuration
      title: StrictModeConfigSparseConfig
    StrictModeConfig:
      type: object
      properties:
        enabled:
          type:
            - boolean
            - "null"
          description: Whether strict mode is enabled for a collection or not.
        max_query_limit:
          type:
            - integer
            - "null"
          description: >-
            Max allowed `limit` parameter for all APIs that don't have their own
            max limit.
        max_timeout:
          type:
            - integer
            - "null"
          description: Max allowed `timeout` parameter.
        unindexed_filtering_retrieve:
          type:
            - boolean
            - "null"
          description: >-
            Allow usage of unindexed fields in retrieval based (e.g. search)
            filters.
        unindexed_filtering_update:
          type:
            - boolean
            - "null"
          description: >-
            Allow usage of unindexed fields in filtered updates (e.g. delete by
            payload).
        search_max_hnsw_ef:
          type:
            - integer
            - "null"
          description: Max HNSW ef value allowed in search parameters.
        search_allow_exact:
          type:
            - boolean
            - "null"
          description: Whether exact search is allowed.
        search_max_oversampling:
          type:
            - number
            - "null"
          format: double
          description: Max oversampling value allowed in search.
        upsert_max_batchsize:
          type:
            - integer
            - "null"
          description: Max batchsize when upserting
        max_collection_vector_size_bytes:
          type:
            - integer
            - "null"
          description: >-
            Max size of a collections vector storage in bytes, ignoring
            replicas.
        read_rate_limit:
          type:
            - integer
            - "null"
          description: Max number of read operations per minute per replica
        write_rate_limit:
          type:
            - integer
            - "null"
          description: Max number of write operations per minute per replica
        max_collection_payload_size_bytes:
          type:
            - integer
            - "null"
          description: Max size of a collections payload storage in bytes
        max_points_count:
          type:
            - integer
            - "null"
          description: Max number of points estimated in a collection
        filter_max_conditions:
          type:
            - integer
            - "null"
          description: Max conditions a filter can have.
        condition_max_size:
          type:
            - integer
            - "null"
          description: Max size of a condition, eg. items in `MatchAny`.
        multivector_config:
          $ref: "#/components/schemas/StrictModeConfigMultivectorConfig"
          description: Multivector strict mode configuration
        sparse_config:
          $ref: "#/components/schemas/StrictModeConfigSparseConfig"
          description: Sparse vector strict mode configuration
        max_payload_index_count:
          type:
            - integer
            - "null"
          description: Max number of payload indexes in a collection
      title: StrictModeConfig
    CreateCollectionStrictModeConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeConfig"
        - description: Any type
      description: Strict-mode config.
      title: CreateCollectionStrictModeConfig
    Payload:
      type: object
      additionalProperties:
        description: Any type
      title: Payload
    CreateCollectionMetadata:
      oneOf:
        - $ref: "#/components/schemas/Payload"
        - description: Any type
      description: >-
        Arbitrary JSON metadata for the collection This can be used to store
        application-specific information such as creation time, migration data,
        inference model info, etc.
      title: CreateCollectionMetadata
    CreateCollection:
      type: object
      properties:
        vectors:
          $ref: "#/components/schemas/VectorsConfig"
        shard_number:
          type:
            - integer
            - "null"
          format: uint
          description: >-
            For auto sharding: Number of shards in collection. - Default is 1
            for standalone, otherwise equal to the number of nodes - Minimum is
            1


            For custom sharding: Number of shards in collection per shard group.
            - Default is 1, meaning that each shard key will be mapped to a
            single shard - Minimum is 1
        sharding_method:
          $ref: "#/components/schemas/CreateCollectionShardingMethod"
          description: >-
            Sharding method Default is Auto - points are distributed across all
            available shards Custom - points are distributed across shards
            according to shard key
        replication_factor:
          type:
            - integer
            - "null"
          format: uint
          description: Number of shards replicas. Default is 1 Minimum is 1
        write_consistency_factor:
          type:
            - integer
            - "null"
          format: uint
          description: >-
            Defines how many replicas should apply the operation for us to
            consider it successful. Increasing this number will make the
            collection more resilient to inconsistencies, but will also make it
            fail if not enough replicas are available. Does not have any
            performance impact.
        on_disk_payload:
          type:
            - boolean
            - "null"
          description: >-
            If true - point's payload will not be stored in memory. It will be
            read from the disk every time it is requested. This setting saves
            RAM by (slightly) increasing the response time. Note: those payload
            values that are involved in filtering and are indexed - remain in
            RAM.


            Default: true
        hnsw_config:
          $ref: "#/components/schemas/CreateCollectionHnswConfig"
          description: >-
            Custom params for HNSW index. If none - values from service
            configuration file are used.
        wal_config:
          $ref: "#/components/schemas/CreateCollectionWalConfig"
          description: >-
            Custom params for WAL. If none - values from service configuration
            file are used.
        optimizers_config:
          $ref: "#/components/schemas/CreateCollectionOptimizersConfig"
          description: >-
            Custom params for Optimizers.  If none - values from service
            configuration file are used.
        quantization_config:
          $ref: "#/components/schemas/CreateCollectionQuantizationConfig"
          description: Quantization parameters. If none - quantization is disabled.
        sparse_vectors:
          type:
            - object
            - "null"
          additionalProperties:
            $ref: "#/components/schemas/SparseVectorParams"
          description: Sparse vector data config.
        strict_mode_config:
          $ref: "#/components/schemas/CreateCollectionStrictModeConfig"
          description: Strict-mode config.
        metadata:
          $ref: "#/components/schemas/CreateCollectionMetadata"
          description: >-
            Arbitrary JSON metadata for the collection This can be used to store
            application-specific information such as creation time, migration
            data, inference model info, etc.
      description: >-
        Operation for creating new collection and (optionally) specify index
        params
      title: CreateCollection
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNamePutResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: CollectionsCollectionNamePutResponsesContentApplicationJsonSchemaUsage
    Collections_create_collection_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNamePutResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          type: boolean
      title: Collections_create_collection_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient, models

client = QdrantClient(url="http://localhost:6333")

client.create_collection(
    collection_name="{collection_name}",
    vectors_config=models.VectorParams(size=100, distance=models.Distance.COSINE),
)

```

```rust
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder};
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client
    .create_collection(
        CreateCollectionBuilder::new("{collection_name}")
            .vectors_config(VectorParamsBuilder::new(100, Distance::Cosine)),
    )
    .await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

import io.qdrant.client.grpc.Collections.Distance;
import io.qdrant.client.grpc.Collections.VectorParams;

QdrantClient client = new QdrantClient(
    QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.createCollectionAsync("{collection_name}",
        VectorParams.newBuilder().setDistance(Distance.Cosine).setSize(100).build()).get();

// Or with sparse vectors

client.createCollectionAsync(
    CreateCollection.newBuilder()
        .setCollectionName("{collection_name}")
        .setSparseVectorsConfig(
            Collections.SparseVectorConfig.newBuilder().putMap(
                "splade-model-name",
                Collections.SparseVectorParams.newBuilder()
                    .setIndex(
                        Collections.SparseIndexConfig
                            .newBuilder()
                            .setOnDisk(false)
                            .build()
                    ).build()
            ).build()
        ).build()
).get();
```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.createCollection("{collection_name}", {
  vectors: { size: 100, distance: "Cosine" },
});

// or with sparse vectors

client.createCollection("{collection_name}", {
  vectors: { size: 100, distance: "Cosine" },
  sparse_vectors: {
    "splade-model-name": {
      index: {
        on_disk: false,
      },
    },
  },
});
```

```go
package client

import (
	"context"

	"github.com/qdrant/go-client/qdrant"
)

func createCollection() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	err = client.CreateCollection(context.Background(), &qdrant.CreateCollection{
		CollectionName: "{collection_name}",
		VectorsConfig: qdrant.NewVectorsConfig(&qdrant.VectorParams{
			Size:     100,
			Distance: qdrant.Distance_Cosine,
		}),
	})
	if err != nil {
		panic(err)
	}
}

```

```csharp
using Qdrant.Client;
using Qdrant.Client.Grpc;

var client = new QdrantClient("localhost", 6334);

await client.CreateCollectionAsync(
	collectionName: "{collection_name}",
	vectorsConfig: new VectorParams { Size = 100, Distance = Distance.Cosine }
);

// Or with sparse vectors

await client.CreateCollectionAsync(
	collectionName: "{collection_name}",
	sparseVectorsConfig: ("splade-model-name", new SparseVectorParams{
        Index = new SparseIndexConfig {
            OnDisk = false,
        }
    })
);
```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Put.new(url)
request["api-key"] = '<apiKey>'
request["Content-Type"] = 'application/json'
request.body = "{}"

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('PUT', 'http://localhost:6333/collections/collection_name', [
  'body' => '{}',
  'headers' => [
    'Content-Type' => 'application/json',
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = [
  "api-key": "<apiKey>",
  "Content-Type": "application/json"
]
let parameters = [] as [String : Any]

let postData = JSONSerialization.data(withJSONObject: parameters, options: [])

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "PUT"
request.allHTTPHeaderFields = headers
request.httpBody = postData as Data

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Delete a collection

DELETE http://localhost:6333/collections/{collection_name}

Drops the specified collection and all associated data in it.

Reference: https://api.qdrant.tech/api-reference/collections/delete-collection

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}:
    delete:
      operationId: delete-collection
      summary: Delete a collection
      description: Drops the specified collection and all associated data in it.
      tags:
        - subpackage_collections
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection to delete
          required: true
          schema:
            type: string
        - name: timeout
          in: query
          description: |
            Wait for operation commit timeout in seconds.
            If timeout is reached - request will return with service error.
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: >-
                  #/components/schemas/Collections_delete_collection_Response_200
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameDeleteResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNameDeleteResponsesContentApplicationJsonSchemaUsage
    Collections_delete_collection_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameDeleteResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          type: boolean
      title: Collections_delete_collection_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.delete_collection(collection_name="{collection_name}")

```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.delete_collection("{collection_name}").await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client =
    new QdrantClient(QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.deleteCollectionAsync("{collection_name}").get();

```

```typescript
import { QdrantClient } from "@qdrant/qdrant-js";

const client = new QdrantClient({ url: "http://127.0.0.1:6333" });

client.deleteCollection("{collection_name}");
```

```go
package client

import (
	"context"

	"github.com/qdrant/go-client/qdrant"
)

func deleteCollection() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	err = client.DeleteCollection(context.Background(), "{collection_name}")
	if err != nil {
		panic(err)
	}
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.DeleteCollectionAsync("{collection_name}");

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Delete.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('DELETE', 'http://localhost:6333/collections/collection_name', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "DELETE"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Update collection parameters

PATCH http://localhost:6333/collections/{collection_name}
Content-Type: application/json

Updates the parameters of the specified collection.

Reference: https://api.qdrant.tech/api-reference/collections/update-collection

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}:
    patch:
      operationId: update-collection
      summary: Update collection parameters
      description: Updates the parameters of the specified collection.
      tags:
        - subpackage_collections
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection to update
          required: true
          schema:
            type: string
        - name: timeout
          in: query
          description: |
            Wait for operation commit timeout in seconds.
            If timeout is reached - request will return with service error.
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: >-
                  #/components/schemas/Collections_update_collection_Response_200
      requestBody:
        description: New parameters
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/UpdateCollection"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HnswConfigDiff:
      type: object
      properties:
        m:
          type:
            - integer
            - "null"
          description: >-
            Number of edges per node in the index graph. Larger the value - more
            accurate the search, more space required.
        ef_construct:
          type:
            - integer
            - "null"
          description: >-
            Number of neighbours to consider during the index building. Larger
            the value - more accurate the search, more time required to build
            the index.
        full_scan_threshold:
          type:
            - integer
            - "null"
          description: >-
            Minimal size threshold (in KiloBytes) below which full-scan is
            preferred over HNSW search. This measures the total size of vectors
            being queried against. When the maximum estimated amount of points
            that a condition satisfies is smaller than `full_scan_threshold_kb`,
            the query planner will use full-scan search instead of HNSW index
            traversal for better performance. Note: 1Kb = 1 vector of size 256
        max_indexing_threads:
          type:
            - integer
            - "null"
          description: >-
            Number of parallel threads used for background index building. If 0
            - automatically select from 8 to 16. Best to keep between 8 and 16
            to prevent likelihood of building broken/inefficient HNSW graphs. On
            small CPUs, less threads are used.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store HNSW index on disk. If set to false, the index will be stored
            in RAM. Default: false
        payload_m:
          type:
            - integer
            - "null"
          description: >-
            Custom M param for additional payload-aware HNSW links. If not set,
            default M will be used.
        inline_storage:
          type:
            - boolean
            - "null"
          description: >-
            Store copies of original and quantized vectors within the HNSW index
            file. Default: false. Enabling this option will trade the search
            speed for disk usage by reducing amount of random seeks during the
            search. Requires quantized vectors to be enabled. Multi-vectors are
            not supported.
      title: HnswConfigDiff
    VectorParamsDiffHnswConfig:
      oneOf:
        - $ref: "#/components/schemas/HnswConfigDiff"
        - description: Any type
      description: Update params for HNSW index. If empty object - it will be unset.
      title: VectorParamsDiffHnswConfig
    ScalarType:
      type: string
      enum:
        - int8
      title: ScalarType
    ScalarQuantizationConfig:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/ScalarType"
        quantile:
          type:
            - number
            - "null"
          format: double
          description: >-
            Quantile for quantization. Expected value range in [0.5, 1.0]. If
            not set - use the whole range of values
        always_ram:
          type:
            - boolean
            - "null"
          description: >-
            If true - quantized vectors always will be stored in RAM, ignoring
            the config of main storage
      required:
        - type
      title: ScalarQuantizationConfig
    ScalarQuantization:
      type: object
      properties:
        scalar:
          $ref: "#/components/schemas/ScalarQuantizationConfig"
      required:
        - scalar
      title: ScalarQuantization
    CompressionRatio:
      type: string
      enum:
        - x4
        - x8
        - x16
        - x32
        - x64
      title: CompressionRatio
    ProductQuantizationConfig:
      type: object
      properties:
        compression:
          $ref: "#/components/schemas/CompressionRatio"
        always_ram:
          type:
            - boolean
            - "null"
      required:
        - compression
      title: ProductQuantizationConfig
    ProductQuantization:
      type: object
      properties:
        product:
          $ref: "#/components/schemas/ProductQuantizationConfig"
      required:
        - product
      title: ProductQuantization
    BinaryQuantizationEncoding:
      type: string
      enum:
        - one_bit
        - two_bits
        - one_and_half_bits
      title: BinaryQuantizationEncoding
    BinaryQuantizationConfigEncoding:
      oneOf:
        - $ref: "#/components/schemas/BinaryQuantizationEncoding"
        - description: Any type
      title: BinaryQuantizationConfigEncoding
    BinaryQuantizationQueryEncoding:
      type: string
      enum:
        - default
        - binary
        - scalar4bits
        - scalar8bits
      title: BinaryQuantizationQueryEncoding
    BinaryQuantizationConfigQueryEncoding:
      oneOf:
        - $ref: "#/components/schemas/BinaryQuantizationQueryEncoding"
        - description: Any type
      description: >-
        Asymmetric quantization configuration allows a query to have different
        quantization than stored vectors. It can increase the accuracy of search
        at the cost of performance.
      title: BinaryQuantizationConfigQueryEncoding
    BinaryQuantizationConfig:
      type: object
      properties:
        always_ram:
          type:
            - boolean
            - "null"
        encoding:
          $ref: "#/components/schemas/BinaryQuantizationConfigEncoding"
        query_encoding:
          $ref: "#/components/schemas/BinaryQuantizationConfigQueryEncoding"
          description: >-
            Asymmetric quantization configuration allows a query to have
            different quantization than stored vectors. It can increase the
            accuracy of search at the cost of performance.
      title: BinaryQuantizationConfig
    BinaryQuantization:
      type: object
      properties:
        binary:
          $ref: "#/components/schemas/BinaryQuantizationConfig"
      required:
        - binary
      title: BinaryQuantization
    Disabled:
      type: string
      enum:
        - Disabled
      title: Disabled
    QuantizationConfigDiff:
      oneOf:
        - $ref: "#/components/schemas/ScalarQuantization"
        - $ref: "#/components/schemas/ProductQuantization"
        - $ref: "#/components/schemas/BinaryQuantization"
        - $ref: "#/components/schemas/Disabled"
      title: QuantizationConfigDiff
    VectorParamsDiffQuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/QuantizationConfigDiff"
        - description: Any type
      description: Update params for quantization. If none - it is left unchanged.
      title: VectorParamsDiffQuantizationConfig
    VectorParamsDiff:
      type: object
      properties:
        hnsw_config:
          $ref: "#/components/schemas/VectorParamsDiffHnswConfig"
          description: Update params for HNSW index. If empty object - it will be unset.
        quantization_config:
          $ref: "#/components/schemas/VectorParamsDiffQuantizationConfig"
          description: Update params for quantization. If none - it is left unchanged.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            If true, vectors are served from disk, improving RAM usage at the
            cost of latency
      title: VectorParamsDiff
    VectorsConfigDiff:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/VectorParamsDiff"
      description: |-
        Vector update params for multiple vectors

        { "vector_name": { "hnsw_config": { "m": 8 } } }
      title: VectorsConfigDiff
    UpdateCollectionVectors:
      oneOf:
        - $ref: "#/components/schemas/VectorsConfigDiff"
        - description: Any type
      description: >-
        Map of vector data parameters to update for each named vector. To update
        parameters in a collection having a single unnamed vector, use an empty
        string as name.
      title: UpdateCollectionVectors
    MaxOptimizationThreadsSetting:
      type: string
      enum:
        - auto
      title: MaxOptimizationThreadsSetting
    MaxOptimizationThreads:
      oneOf:
        - $ref: "#/components/schemas/MaxOptimizationThreadsSetting"
        - type: integer
      title: MaxOptimizationThreads
    OptimizersConfigDiffMaxOptimizationThreads:
      oneOf:
        - $ref: "#/components/schemas/MaxOptimizationThreads"
        - description: Any type
      description: >-
        Max number of threads (jobs) for running optimizations per shard. Note:
        each optimization job will also use `max_indexing_threads` threads by
        itself for index building. If "auto" - have no limit and choose
        dynamically to saturate CPU. If 0 - no optimization threads,
        optimizations will be disabled.
      title: OptimizersConfigDiffMaxOptimizationThreads
    OptimizersConfigDiff:
      type: object
      properties:
        deleted_threshold:
          type:
            - number
            - "null"
          format: double
          description: >-
            The minimal fraction of deleted vectors in a segment, required to
            perform segment optimization
        vacuum_min_vector_number:
          type:
            - integer
            - "null"
          description: >-
            The minimal number of vectors in a segment, required to perform
            segment optimization
        default_segment_number:
          type:
            - integer
            - "null"
          description: >-
            Target amount of segments optimizer will try to keep. Real amount of
            segments may vary depending on multiple parameters: - Amount of
            stored points - Current write RPS


            It is recommended to select default number of segments as a factor
            of the number of search threads, so that each segment would be
            handled evenly by one of the threads If `default_segment_number =
            0`, will be automatically selected by the number of available CPUs
        max_segment_size:
          type:
            - integer
            - "null"
          description: >-
            Do not create segments larger this size (in kilobytes). Large
            segments might require disproportionately long indexation times,
            therefore it makes sense to limit the size of segments.


            If indexation speed have more priority for your - make this
            parameter lower. If search speed is more important - make this
            parameter higher. Note: 1Kb = 1 vector of size 256
        memmap_threshold:
          type:
            - integer
            - "null"
          description: >-
            Maximum size (in kilobytes) of vectors to store in-memory per
            segment. Segments larger than this threshold will be stored as
            read-only memmapped file.


            Memmap storage is disabled by default, to enable it, set this
            threshold to a reasonable value.


            To disable memmap storage, set this to `0`.


            Note: 1Kb = 1 vector of size 256


            Deprecated since Qdrant 1.15.0
        indexing_threshold:
          type:
            - integer
            - "null"
          description: >-
            Maximum size (in kilobytes) of vectors allowed for plain index,
            exceeding this threshold will enable vector indexing


            Default value is 20,000, based on
            <https://github.com/google-research/google-research/blob/master/scann/docs/algorithms.md>.


            To disable vector indexing, set to `0`.


            Note: 1kB = 1 vector of size 256.
        flush_interval_sec:
          type:
            - integer
            - "null"
          format: uint64
          description: Minimum interval between forced flushes.
        max_optimization_threads:
          $ref: "#/components/schemas/OptimizersConfigDiffMaxOptimizationThreads"
          description: >-
            Max number of threads (jobs) for running optimizations per shard.
            Note: each optimization job will also use `max_indexing_threads`
            threads by itself for index building. If "auto" - have no limit and
            choose dynamically to saturate CPU. If 0 - no optimization threads,
            optimizations will be disabled.
        prevent_unoptimized:
          type:
            - boolean
            - "null"
          description: >-
            If this option is set, service will try to prevent creation of large
            unoptimized segments. When enabled, updates may be blocked at
            request level if there are unoptimized segments larger than indexing
            threshold. Updates will be resumed when optimization is completed
            and segments are optimized below the threshold. Using this option
            may lead to increased delay between submitting an update and its
            application. Default is disabled.
      title: OptimizersConfigDiff
    UpdateCollectionOptimizersConfig:
      oneOf:
        - $ref: "#/components/schemas/OptimizersConfigDiff"
        - description: Any type
      description: >-
        Custom params for Optimizers.  If none - it is left unchanged. This
        operation is blocking, it will only proceed once all current
        optimizations are complete
      title: UpdateCollectionOptimizersConfig
    CollectionParamsDiff:
      type: object
      properties:
        replication_factor:
          type:
            - integer
            - "null"
          format: uint
          description: Number of replicas for each shard
        write_consistency_factor:
          type:
            - integer
            - "null"
          format: uint
          description: >-
            Minimal number successful responses from replicas to consider
            operation successful
        read_fan_out_factor:
          type:
            - integer
            - "null"
          format: uint
          description: >-
            Fan-out every read request to these many additional remote nodes
            (and return first available response)
        read_fan_out_delay_ms:
          type:
            - integer
            - "null"
          format: uint64
          description: Delay in milliseconds before sending read requests to remote nodes
        on_disk_payload:
          type:
            - boolean
            - "null"
          description: >-
            If true - point's payload will not be stored in memory. It will be
            read from the disk every time it is requested. This setting saves
            RAM by (slightly) increasing the response time. Note: those payload
            values that are involved in filtering and are indexed - remain in
            RAM.
      title: CollectionParamsDiff
    UpdateCollectionParams:
      oneOf:
        - $ref: "#/components/schemas/CollectionParamsDiff"
        - description: Any type
      description: Collection base params. If none - it is left unchanged.
      title: UpdateCollectionParams
    UpdateCollectionHnswConfig:
      oneOf:
        - $ref: "#/components/schemas/HnswConfigDiff"
        - description: Any type
      description: >-
        HNSW parameters to update for the collection index. If none - it is left
        unchanged.
      title: UpdateCollectionHnswConfig
    UpdateCollectionQuantizationConfig:
      oneOf:
        - $ref: "#/components/schemas/QuantizationConfigDiff"
        - description: Any type
      description: Quantization parameters to update. If none - it is left unchanged.
      title: UpdateCollectionQuantizationConfig
    Datatype:
      type: string
      enum:
        - float32
        - uint8
        - float16
      title: Datatype
    SparseIndexParamsDatatype:
      oneOf:
        - $ref: "#/components/schemas/Datatype"
        - description: Any type
      description: >-
        Defines which datatype should be used for the index. Choosing different
        datatypes allows to optimize memory usage and performance vs accuracy.


        - For `float32` datatype - vectors are stored as single-precision
        floating point numbers, 4 bytes. - For `float16` datatype - vectors are
        stored as half-precision floating point numbers, 2 bytes. - For `uint8`
        datatype - vectors are quantized to unsigned 8-bit integers, 1 byte.
        Quantization to fit byte range `[0, 255]` happens during indexing
        automatically, so the actual vector data does not need to conform to
        this range.
      title: SparseIndexParamsDatatype
    SparseIndexParams:
      type: object
      properties:
        full_scan_threshold:
          type:
            - integer
            - "null"
          description: >-
            We prefer a full scan search upto (excluding) this number of
            vectors.


            Note: this is number of vectors, not KiloBytes.
        on_disk:
          type:
            - boolean
            - "null"
          description: >-
            Store index on disk. If set to false, the index will be stored in
            RAM. Default: false
        datatype:
          $ref: "#/components/schemas/SparseIndexParamsDatatype"
          description: >-
            Defines which datatype should be used for the index. Choosing
            different datatypes allows to optimize memory usage and performance
            vs accuracy.


            - For `float32` datatype - vectors are stored as single-precision
            floating point numbers, 4 bytes. - For `float16` datatype - vectors
            are stored as half-precision floating point numbers, 2 bytes. - For
            `uint8` datatype - vectors are quantized to unsigned 8-bit integers,
            1 byte. Quantization to fit byte range `[0, 255]` happens during
            indexing automatically, so the actual vector data does not need to
            conform to this range.
      description: Configuration for sparse inverted index.
      title: SparseIndexParams
    SparseVectorParamsIndex:
      oneOf:
        - $ref: "#/components/schemas/SparseIndexParams"
        - description: Any type
      description: >-
        Custom params for index. If none - values from collection configuration
        are used.
      title: SparseVectorParamsIndex
    Modifier:
      type: string
      enum:
        - none
        - idf
      description: >-
        If used, include weight modification, which will be applied to sparse
        vectors at query time: None - no modification (default) Idf - inverse
        document frequency, based on statistics of the collection
      title: Modifier
    SparseVectorParamsModifier:
      oneOf:
        - $ref: "#/components/schemas/Modifier"
        - description: Any type
      description: >-
        Configures addition value modifications for sparse vectors. Default:
        none
      title: SparseVectorParamsModifier
    SparseVectorParams:
      type: object
      properties:
        index:
          $ref: "#/components/schemas/SparseVectorParamsIndex"
          description: >-
            Custom params for index. If none - values from collection
            configuration are used.
        modifier:
          $ref: "#/components/schemas/SparseVectorParamsModifier"
          description: >-
            Configures addition value modifications for sparse vectors. Default:
            none
      description: Params of single sparse vector data storage
      title: SparseVectorParams
    SparseVectorsConfig:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/SparseVectorParams"
      title: SparseVectorsConfig
    UpdateCollectionSparseVectors:
      oneOf:
        - $ref: "#/components/schemas/SparseVectorsConfig"
        - description: Any type
      description: Map of sparse vector data parameters to update for each sparse vector.
      title: UpdateCollectionSparseVectors
    StrictModeMultivector:
      type: object
      properties:
        max_vectors:
          type:
            - integer
            - "null"
          description: Max number of vectors in a multivector
      title: StrictModeMultivector
    StrictModeMultivectorConfig:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/StrictModeMultivector"
      title: StrictModeMultivectorConfig
    StrictModeConfigMultivectorConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeMultivectorConfig"
        - description: Any type
      description: Multivector strict mode configuration
      title: StrictModeConfigMultivectorConfig
    StrictModeSparse:
      type: object
      properties:
        max_length:
          type:
            - integer
            - "null"
          description: Max length of sparse vector
      title: StrictModeSparse
    StrictModeSparseConfig:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/StrictModeSparse"
      title: StrictModeSparseConfig
    StrictModeConfigSparseConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeSparseConfig"
        - description: Any type
      description: Sparse vector strict mode configuration
      title: StrictModeConfigSparseConfig
    StrictModeConfig:
      type: object
      properties:
        enabled:
          type:
            - boolean
            - "null"
          description: Whether strict mode is enabled for a collection or not.
        max_query_limit:
          type:
            - integer
            - "null"
          description: >-
            Max allowed `limit` parameter for all APIs that don't have their own
            max limit.
        max_timeout:
          type:
            - integer
            - "null"
          description: Max allowed `timeout` parameter.
        unindexed_filtering_retrieve:
          type:
            - boolean
            - "null"
          description: >-
            Allow usage of unindexed fields in retrieval based (e.g. search)
            filters.
        unindexed_filtering_update:
          type:
            - boolean
            - "null"
          description: >-
            Allow usage of unindexed fields in filtered updates (e.g. delete by
            payload).
        search_max_hnsw_ef:
          type:
            - integer
            - "null"
          description: Max HNSW ef value allowed in search parameters.
        search_allow_exact:
          type:
            - boolean
            - "null"
          description: Whether exact search is allowed.
        search_max_oversampling:
          type:
            - number
            - "null"
          format: double
          description: Max oversampling value allowed in search.
        upsert_max_batchsize:
          type:
            - integer
            - "null"
          description: Max batchsize when upserting
        max_collection_vector_size_bytes:
          type:
            - integer
            - "null"
          description: >-
            Max size of a collections vector storage in bytes, ignoring
            replicas.
        read_rate_limit:
          type:
            - integer
            - "null"
          description: Max number of read operations per minute per replica
        write_rate_limit:
          type:
            - integer
            - "null"
          description: Max number of write operations per minute per replica
        max_collection_payload_size_bytes:
          type:
            - integer
            - "null"
          description: Max size of a collections payload storage in bytes
        max_points_count:
          type:
            - integer
            - "null"
          description: Max number of points estimated in a collection
        filter_max_conditions:
          type:
            - integer
            - "null"
          description: Max conditions a filter can have.
        condition_max_size:
          type:
            - integer
            - "null"
          description: Max size of a condition, eg. items in `MatchAny`.
        multivector_config:
          $ref: "#/components/schemas/StrictModeConfigMultivectorConfig"
          description: Multivector strict mode configuration
        sparse_config:
          $ref: "#/components/schemas/StrictModeConfigSparseConfig"
          description: Sparse vector strict mode configuration
        max_payload_index_count:
          type:
            - integer
            - "null"
          description: Max number of payload indexes in a collection
      title: StrictModeConfig
    UpdateCollectionStrictModeConfig:
      oneOf:
        - $ref: "#/components/schemas/StrictModeConfig"
        - description: Any type
      title: UpdateCollectionStrictModeConfig
    Payload:
      type: object
      additionalProperties:
        description: Any type
      title: Payload
    UpdateCollectionMetadata:
      oneOf:
        - $ref: "#/components/schemas/Payload"
        - description: Any type
      description: >-
        Metadata to update for the collection. If provided, this will merge with
        existing metadata. To remove metadata, set it to an empty object.
      title: UpdateCollectionMetadata
    UpdateCollection:
      type: object
      properties:
        vectors:
          $ref: "#/components/schemas/UpdateCollectionVectors"
          description: >-
            Map of vector data parameters to update for each named vector. To
            update parameters in a collection having a single unnamed vector,
            use an empty string as name.
        optimizers_config:
          $ref: "#/components/schemas/UpdateCollectionOptimizersConfig"
          description: >-
            Custom params for Optimizers.  If none - it is left unchanged. This
            operation is blocking, it will only proceed once all current
            optimizations are complete
        params:
          $ref: "#/components/schemas/UpdateCollectionParams"
          description: Collection base params. If none - it is left unchanged.
        hnsw_config:
          $ref: "#/components/schemas/UpdateCollectionHnswConfig"
          description: >-
            HNSW parameters to update for the collection index. If none - it is
            left unchanged.
        quantization_config:
          $ref: "#/components/schemas/UpdateCollectionQuantizationConfig"
          description: Quantization parameters to update. If none - it is left unchanged.
        sparse_vectors:
          $ref: "#/components/schemas/UpdateCollectionSparseVectors"
          description: >-
            Map of sparse vector data parameters to update for each sparse
            vector.
        strict_mode_config:
          $ref: "#/components/schemas/UpdateCollectionStrictModeConfig"
        metadata:
          $ref: "#/components/schemas/UpdateCollectionMetadata"
          description: >-
            Metadata to update for the collection. If provided, this will merge
            with existing metadata. To remove metadata, set it to an empty
            object.
      description: Operation for updating parameters of the existing collection
      title: UpdateCollection
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNamePatchResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: CollectionsCollectionNamePatchResponsesContentApplicationJsonSchemaUsage
    Collections_update_collection_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNamePatchResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          type: boolean
      title: Collections_update_collection_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.update_collection(
    collection_name="{collection_name}",
    optimizer_config=models.OptimizersConfigDiff(indexing_threshold=10000),
)

```

```rust
use qdrant_client::qdrant::{OptimizersConfigDiffBuilder, UpdateCollectionBuilder};
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client
    .update_collection(
        UpdateCollectionBuilder::new("{collection_name}").optimizers_config(
            OptimizersConfigDiffBuilder::default().indexing_threshold(10_000),
        ),
    )
    .await?;

```

```java
import static io.qdrant.client.ShardKeyFactory.shardKey;

import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

import io.qdrant.client.grpc.Collections.OptimizersConfigDiff;
import io.qdrant.client.grpc.Collections.UpdateCollection;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.updateCollectionAsync(
    UpdateCollection.newBuilder()
        .setCollectionName("{collection_name}")
        .setOptimizersConfig(
            OptimizersConfigDiff.newBuilder().setIndexingThreshold(10000).build())
        .build());

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.updateCollection("{collection_name}", {
  optimizers_config: {
    indexing_threshold: 10000,
  },
});
```

```go
package client

import (
	"context"

	"github.com/qdrant/go-client/qdrant"
)

func updateCollection() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	threshold := uint64(10000)
	err = client.UpdateCollection(context.Background(), &qdrant.UpdateCollection{
		CollectionName: "{collection_name}",
		OptimizersConfig: &qdrant.OptimizersConfigDiff{
			IndexingThreshold: &threshold,
		},
	})
	if err != nil {
		panic(err)
	}
}

```

```csharp
using Qdrant.Client;
using Qdrant.Client.Grpc;

var client = new QdrantClient("localhost", 6334);

await client.UpdateCollectionAsync(
  collectionName: "{collection_name}",
  optimizersConfig: new OptimizersConfigDiff { IndexingThreshold = 10000 }
);

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Patch.new(url)
request["api-key"] = '<apiKey>'
request["Content-Type"] = 'application/json'
request.body = "{}"

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('PATCH', 'http://localhost:6333/collections/collection_name', [
  'body' => '{}',
  'headers' => [
    'Content-Type' => 'application/json',
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = [
  "api-key": "<apiKey>",
  "Content-Type": "application/json"
]
let parameters = [] as [String : Any]

let postData = JSONSerialization.data(withJSONObject: parameters, options: [])

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "PATCH"
request.allHTTPHeaderFields = headers
request.httpBody = postData as Data

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# List all collections

GET http://localhost:6333/collections

Returns a list of all existing collections.

Reference: https://api.qdrant.tech/api-reference/collections/get-collections

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections:
    get:
      operationId: get-collections
      summary: List all collections
      description: Returns a list of all existing collections.
      tags:
        - subpackage_collections
      parameters:
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Collections_get_collections_Response_200"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsGetResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: CollectionsGetResponsesContentApplicationJsonSchemaUsage
    CollectionDescription:
      type: object
      properties:
        name:
          type: string
      required:
        - name
      title: CollectionDescription
    CollectionsResponse:
      type: object
      properties:
        collections:
          type: array
          items:
            $ref: "#/components/schemas/CollectionDescription"
      required:
        - collections
      title: CollectionsResponse
    Collections_get_collections_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsGetResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/CollectionsResponse"
      title: Collections_get_collections_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.get_collections()

```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.list_collections().await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.listCollectionsAsync().get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.getCollections();
```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func listCollections() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	collections, err := client.ListCollections(context.Background())
	if err != nil {
		panic(err)
	}
	fmt.Println("Collections: ", collections)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.ListCollectionsAsync();

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Get.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('GET', 'http://localhost:6333/collections', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "GET"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Check collection existence

GET http://localhost:6333/collections/{collection_name}/exists

Checks whether the specified collection exists.

Reference: https://api.qdrant.tech/api-reference/collections/collection-exists

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/exists:
    get:
      operationId: collection-exists
      summary: Check collection existence
      description: Checks whether the specified collection exists.
      tags:
        - subpackage_collections
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection
          required: true
          schema:
            type: string
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: >-
                  #/components/schemas/Collections_collection_exists_Response_200
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameExistsGetResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNameExistsGetResponsesContentApplicationJsonSchemaUsage
    CollectionExistence:
      type: object
      properties:
        exists:
          type: boolean
      required:
        - exists
      description: >-
        State of existence of a collection, true = exists, false = does not
        exist
      title: CollectionExistence
    Collections_collection_exists_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameExistsGetResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/CollectionExistence"
      title: Collections_collection_exists_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.collection_exists(collection_name="{collection_name}")
```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.collection_exists("{collection_name}").await?;

```

```java
import static io.qdrant.client.ConditionFactory.matchKeyword;

import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.collectionExistsAsync("{collection_name}").get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.collectionExists("{collection_name}");
```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func collectionExists() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	exists, err := client.CollectionExists(context.Background(), "{collection_name}")
	if err != nil {
		panic(err)
	}
	fmt.Println("Collection exists: ", exists)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.CollectionExistsAsync("{collection_name}");

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/exists")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Get.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('GET', 'http://localhost:6333/collections/collection_name/exists', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/exists")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "GET"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Get optimization progress

GET http://localhost:6333/collections/{collection_name}/optimizations

Get progress of ongoing and completed optimizations for a collection

Reference: https://api.qdrant.tech/api-reference/collections/get-optimizations

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/optimizations:
    get:
      operationId: get-optimizations
      summary: Get optimization progress
      description: Get progress of ongoing and completed optimizations for a collection
      tags:
        - subpackage_collections
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection
          required: true
          schema:
            type: string
        - name: with
          in: query
          description: |-
            Comma-separated list of optional fields to include in the response.
            Possible values: queued, completed, idle_segments.
          required: false
          schema:
            type: string
        - name: completed_limit
          in: query
          description: |-
            Maximum number of completed optimizations to return.
            Ignored if `completed` is not in the `with` parameter.
          required: false
          schema:
            type: integer
            default: 16
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: >-
                  #/components/schemas/Collections_get_optimizations_Response_200
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameOptimizationsGetResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNameOptimizationsGetResponsesContentApplicationJsonSchemaUsage
    OptimizationsSummary:
      type: object
      properties:
        queued_optimizations:
          type: integer
          description: >-
            Number of pending optimizations in the queue. Each optimization will
            take one or more unoptimized segments and produce one optimized
            segment.
        queued_segments:
          type: integer
          description: Number of unoptimized segments in the queue.
        queued_points:
          type: integer
          description: Number of points in unoptimized segments in the queue.
        idle_segments:
          type: integer
          description: Number of segments that don't require optimization.
      required:
        - queued_optimizations
        - queued_segments
        - queued_points
        - idle_segments
      title: OptimizationsSummary
    TrackerStatus0:
      type: string
      enum:
        - optimizing
        - done
      title: TrackerStatus0
    TrackerStatus1:
      type: object
      properties:
        cancelled:
          type: string
      required:
        - cancelled
      title: TrackerStatus1
    TrackerStatus2:
      type: object
      properties:
        error:
          type: string
      required:
        - error
      title: TrackerStatus2
    TrackerStatus:
      oneOf:
        - $ref: "#/components/schemas/TrackerStatus0"
        - $ref: "#/components/schemas/TrackerStatus1"
        - $ref: "#/components/schemas/TrackerStatus2"
      description: Represents the current state of the optimizer being tracked
      title: TrackerStatus
    OptimizationSegmentInfo:
      type: object
      properties:
        uuid:
          type: string
          format: uuid
          description: Unique identifier of the segment.
        points_count:
          type: integer
          description: Number of non-deleted points in the segment.
      required:
        - uuid
        - points_count
      title: OptimizationSegmentInfo
    ProgressTree:
      type: object
      properties:
        name:
          type: string
          description: Name of the operation.
        started_at:
          type:
            - string
            - "null"
          format: date-time
          description: When the operation started.
        finished_at:
          type:
            - string
            - "null"
          format: date-time
          description: When the operation finished.
        duration_sec:
          type:
            - number
            - "null"
          format: double
          description: For finished operations, how long they took, in seconds.
        done:
          type:
            - integer
            - "null"
          format: uint64
          description: Number of completed units of work, if applicable.
        total:
          type:
            - integer
            - "null"
          format: uint64
          description: Total number of units of work, if applicable and known.
        children:
          type: array
          items:
            $ref: "#/components/schemas/ProgressTree"
          description: Child operations.
      required:
        - name
      title: ProgressTree
    Optimization:
      type: object
      properties:
        uuid:
          type: string
          format: uuid
          description: >-
            Unique identifier of the optimization process.


            After the optimization is complete, a new segment will be created
            with this UUID.
        optimizer:
          type: string
          description: Name of the optimizer that performed this optimization.
        status:
          $ref: "#/components/schemas/TrackerStatus"
        segments:
          type: array
          items:
            $ref: "#/components/schemas/OptimizationSegmentInfo"
          description: >-
            Segments being optimized.


            After the optimization is complete, these segments will be replaced
            by the new optimized segment.
        progress:
          $ref: "#/components/schemas/ProgressTree"
      required:
        - uuid
        - optimizer
        - status
        - segments
        - progress
      title: Optimization
    PendingOptimization:
      type: object
      properties:
        optimizer:
          type: string
          description: Name of the optimizer that scheduled this optimization.
        segments:
          type: array
          items:
            $ref: "#/components/schemas/OptimizationSegmentInfo"
          description: Segments that will be optimized.
      required:
        - optimizer
        - segments
      title: PendingOptimization
    OptimizationsResponse:
      type: object
      properties:
        summary:
          $ref: "#/components/schemas/OptimizationsSummary"
        running:
          type: array
          items:
            $ref: "#/components/schemas/Optimization"
          description: Currently running optimizations.
        queued:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/PendingOptimization"
          description: >-
            An estimated queue of pending optimizations. Requires
            `?with=queued`.
        completed:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/Optimization"
          description: >-
            Completed optimizations. Requires `?with=completed`. Limited by
            `?completed_limit=N`.
        idle_segments:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/OptimizationSegmentInfo"
          description: >-
            Segments that don't require optimization. Requires
            `?with=idle_segments`.
      required:
        - summary
        - running
      description: Optimizations progress for the collection
      title: OptimizationsResponse
    Collections_get_optimizations_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameOptimizationsGetResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/OptimizationsResponse"
      title: Collections_get_optimizations_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
import requests

url = "http://localhost:6333/collections/collection_name/optimizations"

headers = {"api-key": "<apiKey>"}

response = requests.get(url, headers=headers)

print(response.json())
```

```javascript
const url = "http://localhost:6333/collections/collection_name/optimizations";
const options = { method: "GET", headers: { "api-key": "<apiKey>" } };

try {
  const response = await fetch(url, options);
  const data = await response.json();
  console.log(data);
} catch (error) {
  console.error(error);
}
```

```go
package main

import (
	"fmt"
	"net/http"
	"io"
)

func main() {

	url := "http://localhost:6333/collections/collection_name/optimizations"

	req, _ := http.NewRequest("GET", url, nil)

	req.Header.Add("api-key", "<apiKey>")

	res, _ := http.DefaultClient.Do(req)

	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)

	fmt.Println(res)
	fmt.Println(string(body))

}
```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/optimizations")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Get.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```java
import com.mashape.unirest.http.HttpResponse;
import com.mashape.unirest.http.Unirest;

HttpResponse<String> response = Unirest.get("http://localhost:6333/collections/collection_name/optimizations")
  .header("api-key", "<apiKey>")
  .asString();
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('GET', 'http://localhost:6333/collections/collection_name/optimizations', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```csharp
using RestSharp;

var client = new RestClient("http://localhost:6333/collections/collection_name/optimizations");
var request = new RestRequest(Method.GET);
request.AddHeader("api-key", "<apiKey>");
IRestResponse response = client.Execute(request);
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/optimizations")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "GET"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Create payload index

PUT http://localhost:6333/collections/{collection_name}/index
Content-Type: application/json

Creates a payload index for a field in the specified collection.

Reference: https://api.qdrant.tech/api-reference/indexes/create-field-index

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/index:
    put:
      operationId: create-field-index
      summary: Create payload index
      description: Creates a payload index for a field in the specified collection.
      tags:
        - subpackage_indexes
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection
          required: true
          schema:
            type: string
        - name: wait
          in: query
          description: If true, wait for changes to actually happen
          required: false
          schema:
            type: boolean
        - name: ordering
          in: query
          description: define ordering guarantees for the operation
          required: false
          schema:
            $ref: "#/components/schemas/WriteOrdering"
        - name: timeout
          in: query
          description: Timeout for the operation
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Indexes_create_field_index_Response_200"
      requestBody:
        description: Field name
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/CreateFieldIndex"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    WriteOrdering:
      type: string
      enum:
        - weak
        - medium
        - strong
      description: >-
        Defines write ordering guarantees for collection operations


        * `weak` - write operations may be reordered, works faster, default


        * `medium` - write operations go through dynamically selected leader,
        may be inconsistent for a short period of time in case of leader change


        * `strong` - Write operations go through the permanent leader,
        consistent, but may be unavailable if leader is down
      title: WriteOrdering
    PayloadSchemaType:
      type: string
      enum:
        - keyword
        - integer
        - float
        - geo
        - text
        - bool
        - datetime
        - uuid
      description: All possible names of payload types
      title: PayloadSchemaType
    KeywordIndexType:
      type: string
      enum:
        - keyword
      title: KeywordIndexType
    KeywordIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/KeywordIndexType"
        is_tenant:
          type:
            - boolean
            - "null"
          description: "If true - used for tenant optimization. Default: false."
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: KeywordIndexParams
    IntegerIndexType:
      type: string
      enum:
        - integer
      title: IntegerIndexType
    IntegerIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/IntegerIndexType"
        lookup:
          type:
            - boolean
            - "null"
          description: If true - support direct lookups. Default is true.
        range:
          type:
            - boolean
            - "null"
          description: If true - support ranges filters. Default is true.
        is_principal:
          type:
            - boolean
            - "null"
          description: >-
            If true - use this key to organize storage of the collection data.
            This option assumes that this key will be used in majority of
            filtered requests. Default is false.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false. Default is false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: IntegerIndexParams
    FloatIndexType:
      type: string
      enum:
        - float
      title: FloatIndexType
    FloatIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/FloatIndexType"
        is_principal:
          type:
            - boolean
            - "null"
          description: >-
            If true - use this key to organize storage of the collection data.
            This option assumes that this key will be used in majority of
            filtered requests.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: FloatIndexParams
    GeoIndexType:
      type: string
      enum:
        - geo
      title: GeoIndexType
    GeoIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/GeoIndexType"
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: GeoIndexParams
    TextIndexType:
      type: string
      enum:
        - text
      title: TextIndexType
    TokenizerType:
      type: string
      enum:
        - prefix
        - whitespace
        - word
        - multilingual
      title: TokenizerType
    Language:
      type: string
      enum:
        - arabic
        - azerbaijani
        - basque
        - bengali
        - catalan
        - chinese
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hebrew
        - hinglish
        - hungarian
        - indonesian
        - italian
        - japanese
        - kazakh
        - nepali
        - norwegian
        - portuguese
        - romanian
        - russian
        - slovene
        - spanish
        - swedish
        - tajik
        - turkish
      title: Language
    StopwordsSet:
      type: object
      properties:
        languages:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/Language"
          description: >-
            Set of languages to use for stopwords. Multiple pre-defined lists of
            stopwords can be combined.
        custom:
          type:
            - array
            - "null"
          items:
            type: string
          description: Custom stopwords set. Will be merged with the languages set.
      title: StopwordsSet
    StopwordsInterface:
      oneOf:
        - $ref: "#/components/schemas/Language"
        - $ref: "#/components/schemas/StopwordsSet"
      title: StopwordsInterface
    TextIndexParamsStopwords:
      oneOf:
        - $ref: "#/components/schemas/StopwordsInterface"
        - description: Any type
      description: >-
        Ignore this set of tokens. Can select from predefined languages and/or
        provide a custom set.
      title: TextIndexParamsStopwords
    Snowball:
      type: string
      enum:
        - snowball
      title: Snowball
    SnowballLanguage:
      type: string
      enum:
        - arabic
        - armenian
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hungarian
        - italian
        - norwegian
        - portuguese
        - romanian
        - russian
        - spanish
        - swedish
        - tamil
        - turkish
      description: Languages supported by snowball stemmer.
      title: SnowballLanguage
    SnowballParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/Snowball"
        language:
          $ref: "#/components/schemas/SnowballLanguage"
      required:
        - type
        - language
      title: SnowballParams
    StemmingAlgorithm:
      oneOf:
        - $ref: "#/components/schemas/SnowballParams"
      description: Different stemming algorithms with their configs.
      title: StemmingAlgorithm
    TextIndexParamsStemmer:
      oneOf:
        - $ref: "#/components/schemas/StemmingAlgorithm"
        - description: Any type
      description: "Algorithm for stemming. Default: disabled."
      title: TextIndexParamsStemmer
    TextIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/TextIndexType"
        tokenizer:
          $ref: "#/components/schemas/TokenizerType"
        min_token_len:
          type:
            - integer
            - "null"
          description: Minimum characters to be tokenized.
        max_token_len:
          type:
            - integer
            - "null"
          description: Maximum characters to be tokenized.
        lowercase:
          type:
            - boolean
            - "null"
          description: "If true, lowercase all tokens. Default: true."
        ascii_folding:
          type:
            - boolean
            - "null"
          description: >-
            If true, normalize tokens by folding accented characters to ASCII
            (e.g., "ação" -> "acao"). Default: false.
        phrase_matching:
          type:
            - boolean
            - "null"
          description: "If true, support phrase matching. Default: false."
        stopwords:
          $ref: "#/components/schemas/TextIndexParamsStopwords"
          description: >-
            Ignore this set of tokens. Can select from predefined languages
            and/or provide a custom set.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        stemmer:
          $ref: "#/components/schemas/TextIndexParamsStemmer"
          description: "Algorithm for stemming. Default: disabled."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: TextIndexParams
    BoolIndexType:
      type: string
      enum:
        - bool
      title: BoolIndexType
    BoolIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/BoolIndexType"
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: BoolIndexParams
    DatetimeIndexType:
      type: string
      enum:
        - datetime
      title: DatetimeIndexType
    DatetimeIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/DatetimeIndexType"
        is_principal:
          type:
            - boolean
            - "null"
          description: >-
            If true - use this key to organize storage of the collection data.
            This option assumes that this key will be used in majority of
            filtered requests.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: DatetimeIndexParams
    UuidIndexType:
      type: string
      enum:
        - uuid
      title: UuidIndexType
    UuidIndexParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/UuidIndexType"
        is_tenant:
          type:
            - boolean
            - "null"
          description: If true - used for tenant optimization.
        on_disk:
          type:
            - boolean
            - "null"
          description: "If true, store the index on disk. Default: false."
        enable_hnsw:
          type:
            - boolean
            - "null"
          description: >-
            Enable HNSW graph building for this payload field. If true, builds
            additional HNSW links (Need payload_m > 0). Default: true.
      required:
        - type
      title: UuidIndexParams
    PayloadSchemaParams:
      oneOf:
        - $ref: "#/components/schemas/KeywordIndexParams"
        - $ref: "#/components/schemas/IntegerIndexParams"
        - $ref: "#/components/schemas/FloatIndexParams"
        - $ref: "#/components/schemas/GeoIndexParams"
        - $ref: "#/components/schemas/TextIndexParams"
        - $ref: "#/components/schemas/BoolIndexParams"
        - $ref: "#/components/schemas/DatetimeIndexParams"
        - $ref: "#/components/schemas/UuidIndexParams"
      description: Payload type with parameters
      title: PayloadSchemaParams
    PayloadFieldSchema:
      oneOf:
        - $ref: "#/components/schemas/PayloadSchemaType"
        - $ref: "#/components/schemas/PayloadSchemaParams"
      title: PayloadFieldSchema
    CreateFieldIndexFieldSchema:
      oneOf:
        - $ref: "#/components/schemas/PayloadFieldSchema"
        - description: Any type
      title: CreateFieldIndexFieldSchema
    CreateFieldIndex:
      type: object
      properties:
        field_name:
          type: string
        field_schema:
          $ref: "#/components/schemas/CreateFieldIndexFieldSchema"
      required:
        - field_name
      title: CreateFieldIndex
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameIndexPutResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNameIndexPutResponsesContentApplicationJsonSchemaUsage
    UpdateStatus:
      type: string
      enum:
        - acknowledged
        - completed
        - wait_timeout
      description: >-
        `Acknowledged` - Request is saved to WAL and will be process in a queue.
        `Completed` - Request is completed, changes are actual. `WaitTimeout` -
        Request is waiting for timeout.
      title: UpdateStatus
    UpdateResult:
      type: object
      properties:
        operation_id:
          type:
            - integer
            - "null"
          format: uint64
          description: Sequential number of the operation
        status:
          $ref: "#/components/schemas/UpdateStatus"
      required:
        - status
      title: UpdateResult
    Indexes_create_field_index_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameIndexPutResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/UpdateResult"
      title: Indexes_create_field_index_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.create_payload_index(
    collection_name="{collection_name}",
    field_name="name_of_the_field_to_index",
    field_schema="keyword",
)

```

```rust
use qdrant_client::qdrant::{CreateFieldIndexCollectionBuilder, FieldType};
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client
    .create_field_index(
        CreateFieldIndexCollectionBuilder::new(
            "{collection_name}",
            "{field_name}",
            FieldType::Keyword,
        ),
    )
    .await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

import io.qdrant.client.grpc.Collections.PayloadSchemaType;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.createPayloadIndexAsync(
                "{collection_name}",
                "{field_name}",
                PayloadSchemaType.Keyword,
                null,
                true,
                null,
                null);

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.createPayloadIndex("{collection_name}", {
  field_name: "{field_name}",
  field_schema: "keyword",
});
```

```go
package client

import (
	"context"

	"github.com/qdrant/go-client/qdrant"
)

func createFieldIndex() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	_, err = client.CreateFieldIndex(context.Background(), &qdrant.CreateFieldIndexCollection{
		CollectionName: "{collection_name}",
		FieldName:      "name_of_the_field_to_index",
		FieldType:      qdrant.FieldType_FieldTypeKeyword.Enum(),
	})
	if err != nil {
		panic(err)
	}
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.CreatePayloadIndexAsync(
  collectionName: "{collection_name}",
  fieldName: "name_of_the_field_to_index"
);

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/index")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Put.new(url)
request["api-key"] = '<apiKey>'
request["Content-Type"] = 'application/json'
request.body = "{\n  \"field_name\": \"string\"\n}"

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('PUT', 'http://localhost:6333/collections/collection_name/index', [
  'body' => '{
  "field_name": "string"
}',
  'headers' => [
    'Content-Type' => 'application/json',
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = [
  "api-key": "<apiKey>",
  "Content-Type": "application/json"
]
let parameters = ["field_name": "string"] as [String : Any]

let postData = JSONSerialization.data(withJSONObject: parameters, options: [])

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/index")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "PUT"
request.allHTTPHeaderFields = headers
request.httpBody = postData as Data

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Delete payload index

DELETE http://localhost:6333/collections/{collection_name}/index/{field_name}

Deletes a payload index for a field in the specified collection.

Reference: https://api.qdrant.tech/api-reference/indexes/delete-field-index

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/index/{field_name}:
    delete:
      operationId: delete-field-index
      summary: Delete payload index
      description: Deletes a payload index for a field in the specified collection.
      tags:
        - subpackage_indexes
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection
          required: true
          schema:
            type: string
        - name: field_name
          in: path
          description: Name of the field where to delete the index
          required: true
          schema:
            type: string
        - name: wait
          in: query
          description: If true, wait for changes to actually happen
          required: false
          schema:
            type: boolean
        - name: ordering
          in: query
          description: define ordering guarantees for the operation
          required: false
          schema:
            $ref: "#/components/schemas/WriteOrdering"
        - name: timeout
          in: query
          description: Timeout for the operation
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Indexes_delete_field_index_Response_200"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    WriteOrdering:
      type: string
      enum:
        - weak
        - medium
        - strong
      description: >-
        Defines write ordering guarantees for collection operations


        * `weak` - write operations may be reordered, works faster, default


        * `medium` - write operations go through dynamically selected leader,
        may be inconsistent for a short period of time in case of leader change


        * `strong` - Write operations go through the permanent leader,
        consistent, but may be unavailable if leader is down
      title: WriteOrdering
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameIndexFieldNameDeleteResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNameIndexFieldNameDeleteResponsesContentApplicationJsonSchemaUsage
    UpdateStatus:
      type: string
      enum:
        - acknowledged
        - completed
        - wait_timeout
      description: >-
        `Acknowledged` - Request is saved to WAL and will be process in a queue.
        `Completed` - Request is completed, changes are actual. `WaitTimeout` -
        Request is waiting for timeout.
      title: UpdateStatus
    UpdateResult:
      type: object
      properties:
        operation_id:
          type:
            - integer
            - "null"
          format: uint64
          description: Sequential number of the operation
        status:
          $ref: "#/components/schemas/UpdateStatus"
      required:
        - status
      title: UpdateResult
    Indexes_delete_field_index_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameIndexFieldNameDeleteResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/UpdateResult"
      title: Indexes_delete_field_index_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.delete_payload_index("{collection_name}", "{field_name}");

```

```rust
use qdrant_client::qdrant::DeleteFieldIndexCollectionBuilder;
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client
    .delete_field_index(DeleteFieldIndexCollectionBuilder::new(
        "{collection_name}",
        "{field_name}",
    ))
    .await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.deletePayloadIndexAsync("{collection_name}", "{field_name}", true, null, null).get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.deletePayloadIndex("{collection_name}", "{field_name}");
```

```go
package client

import (
	"context"

	"github.com/qdrant/go-client/qdrant"
)

func deleteFieldIndex() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	_, err = client.DeleteFieldIndex(context.Background(), &qdrant.DeleteFieldIndexCollection{
		CollectionName: "{collection_name}",
		FieldName:      "{field_name}",
	})
	if err != nil {
		panic(err)
	}
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.DeletePayloadIndexAsync(
  collectionName: "{collection_name}",
  fieldName: "name_of_the_field_to_index"
);

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/index/field_name")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Delete.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('DELETE', 'http://localhost:6333/collections/collection_name/index/field_name', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/index/field_name")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "DELETE"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# List all snapshots (collection)

GET http://localhost:6333/collections/{collection_name}/snapshots

Retrieves a list of all snapshots for a specified collection.

Reference: https://api.qdrant.tech/api-reference/snapshots/list-snapshots

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/snapshots:
    get:
      operationId: list-snapshots
      summary: List all snapshots (collection)
      description: Retrieves a list of all snapshots for a specified collection.
      tags:
        - subpackage_snapshots
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection
          required: true
          schema:
            type: string
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Snapshots_list_snapshots_Response_200"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNameSnapshotsGetResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNameSnapshotsGetResponsesContentApplicationJsonSchemaUsage
    SnapshotDescription:
      type: object
      properties:
        name:
          type: string
        creation_time:
          type:
            - string
            - "null"
          format: partial-date-time
        size:
          type: integer
          format: uint64
        checksum:
          type:
            - string
            - "null"
      required:
        - name
        - size
      title: SnapshotDescription
    Snapshots_list_snapshots_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNameSnapshotsGetResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          type: array
          items:
            $ref: "#/components/schemas/SnapshotDescription"
      title: Snapshots_list_snapshots_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.list_snapshots(collection_name="{collection_name}")

```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.list_snapshots("{collection_name}").await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.listSnapshotAsync("{collection_name}").get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.listSnapshots("{collection_name}");
```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func listSnapshots() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	snapshots, err := client.ListSnapshots(context.Background(), "{collection_name}")
	if err != nil {
		panic(err)
	}
	fmt.Println("Snapshots: ", snapshots)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.ListSnapshotsAsync("{collection_name}");

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/snapshots")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Get.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('GET', 'http://localhost:6333/collections/collection_name/snapshots', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/snapshots")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "GET"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Create a snapshot (collection)

POST http://localhost:6333/collections/{collection_name}/snapshots

Creates a new snapshot for a specified collection.

Reference: https://api.qdrant.tech/api-reference/snapshots/create-snapshot

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/snapshots:
    post:
      operationId: create-snapshot
      summary: Create a snapshot (collection)
      description: Creates a new snapshot for a specified collection.
      tags:
        - subpackage_snapshots
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection for which to create a snapshot
          required: true
          schema:
            type: string
        - name: wait
          in: query
          description: >-
            If true, wait for changes to actually happen. If false - let changes
            happen in background. Default is true.
          required: false
          schema:
            type: boolean
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Snapshots_create_snapshot_Response_200"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    SnapshotDescription:
      type: object
      properties:
        name:
          type: string
        creation_time:
          type:
            - string
            - "null"
          format: partial-date-time
        size:
          type: integer
          format: uint64
        checksum:
          type:
            - string
            - "null"
      required:
        - name
        - size
      title: SnapshotDescription
    Snapshots_create_snapshot_Response_200:
      type: object
      properties:
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/SnapshotDescription"
      title: Snapshots_create_snapshot_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.create_snapshot(collection_name="{collection_name}")

```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.create_snapshot("{collection_name}").await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.createSnapshotAsync("{collection_name}").get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.createSnapshot("{collection_name}");
```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func createSnapshot() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	snapshot, err := client.CreateSnapshot(context.Background(), "{collection_name}")
	if err != nil {
		panic(err)
	}
	fmt.Println("Snapshot created: ", snapshot.Name)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.CreateSnapshotAsync("{collection_name}");

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/snapshots")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Post.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('POST', 'http://localhost:6333/collections/collection_name/snapshots', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/snapshots")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "POST"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Create a snapshot (storage)

POST http://localhost:6333/snapshots

Creates a new snapshot of the entire storage.

Reference: https://api.qdrant.tech/api-reference/snapshots/create-full-snapshot

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /snapshots:
    post:
      operationId: create-full-snapshot
      summary: Create a snapshot (storage)
      description: Creates a new snapshot of the entire storage.
      tags:
        - subpackage_snapshots
      parameters:
        - name: wait
          in: query
          description: >-
            If true, wait for changes to actually happen. If false - let changes
            happen in background. Default is true.
          required: false
          schema:
            type: boolean
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: >-
                  #/components/schemas/Snapshots_create_full_snapshot_Response_200
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    SnapshotDescription:
      type: object
      properties:
        name:
          type: string
        creation_time:
          type:
            - string
            - "null"
          format: partial-date-time
        size:
          type: integer
          format: uint64
        checksum:
          type:
            - string
            - "null"
      required:
        - name
        - size
      title: SnapshotDescription
    Snapshots_create_full_snapshot_Response_200:
      type: object
      properties:
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/SnapshotDescription"
      title: Snapshots_create_full_snapshot_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient

client = QdrantClient(url="http://localhost:6333")

client.create_full_snapshot()

```

```rust
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.create_full_snapshot().await?;

```

```java
import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;

QdrantClient client = new QdrantClient(
                QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client.createFullSnapshotAsync().get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.createFullSnapshot();
```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func createFullSnapshot() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	snapshot, err := client.CreateFullSnapshot(context.Background())
	if err != nil {
		panic(err)
	}
	fmt.Println("Snapshot created: ", snapshot.Name)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.CreateFullSnapshotAsync();

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/snapshots")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Post.new(url)
request["api-key"] = '<apiKey>'

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('POST', 'http://localhost:6333/snapshots', [
  'headers' => [
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = ["api-key": "<apiKey>"]

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/snapshots")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "POST"
request.allHTTPHeaderFields = headers

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```

---

# Query points

POST http://localhost:6333/collections/{collection_name}/points/query
Content-Type: application/json

Universally query points. This endpoint covers all capabilities of search, recommend, discover, filters. But also enables hybrid and multi-stage queries.

Reference: https://api.qdrant.tech/api-reference/search/query-points

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/points/query:
    post:
      operationId: query-points
      summary: Query points
      description: >-
        Universally query points. This endpoint covers all capabilities of
        search, recommend, discover, filters. But also enables hybrid and
        multi-stage queries.
      tags:
        - subpackage_search
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection to query
          required: true
          schema:
            type: string
        - name: consistency
          in: query
          description: Define read consistency guarantees for the operation
          required: false
          schema:
            $ref: "#/components/schemas/ReadConsistency"
        - name: timeout
          in: query
          description: If set, overrides global timeout for this request. Unit is seconds.
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        "200":
          description: successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Search_query_points_Response_200"
      requestBody:
        description: Describes the query to make to the collection
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/QueryRequest"
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    ReadConsistencyType:
      type: string
      enum:
        - majority
        - quorum
        - all
      description: >-
        * `majority` - send N/2+1 random request and return points, which
        present on all of them


        * `quorum` - send requests to all nodes and return points which present
        on majority of nodes


        * `all` - send requests to all nodes and return points which present on
        all nodes
      title: ReadConsistencyType
    ReadConsistency:
      oneOf:
        - type: integer
        - $ref: "#/components/schemas/ReadConsistencyType"
      description: >-
        Read consistency parameter


        Defines how many replicas should be queried to get the result


        * `N` - send N random request and return points, which present on all of
        them


        * `majority` - send N/2+1 random request and return points, which
        present on all of them


        * `quorum` - send requests to all nodes and return points which present
        on majority of them


        * `all` - send requests to all nodes and return points which present on
        all of them


        Default value is `Factor(1)`
      title: ReadConsistency
    ShardKey:
      oneOf:
        - type: string
        - type: integer
          format: uint64
      title: ShardKey
    ShardKeySelector1:
      type: array
      items:
        $ref: "#/components/schemas/ShardKey"
      title: ShardKeySelector1
    ShardKeyWithFallback:
      type: object
      properties:
        target:
          $ref: "#/components/schemas/ShardKey"
        fallback:
          $ref: "#/components/schemas/ShardKey"
      required:
        - target
        - fallback
      title: ShardKeyWithFallback
    ShardKeySelector:
      oneOf:
        - $ref: "#/components/schemas/ShardKey"
        - $ref: "#/components/schemas/ShardKeySelector1"
        - $ref: "#/components/schemas/ShardKeyWithFallback"
      title: ShardKeySelector
    QueryRequestShardKey:
      oneOf:
        - $ref: "#/components/schemas/ShardKeySelector"
        - description: Any type
      title: QueryRequestShardKey
    PrefetchPrefetch1:
      type: array
      items:
        $ref: "#/components/schemas/Prefetch"
      title: PrefetchPrefetch1
    PrefetchPrefetch:
      oneOf:
        - $ref: "#/components/schemas/Prefetch"
        - $ref: "#/components/schemas/PrefetchPrefetch1"
        - description: Any type
      description: >-
        Sub-requests to perform first. If present, the query will be performed
        on the results of the prefetches.
      title: PrefetchPrefetch
    SparseVector:
      type: object
      properties:
        indices:
          type: array
          items:
            type: integer
            format: uint
          description: Indices must be unique
        values:
          type: array
          items:
            type: number
            format: double
          description: Values and indices must be the same length
      required:
        - indices
        - values
      description: Sparse vector structure
      title: SparseVector
    ExtendedPointId:
      oneOf:
        - type: integer
          format: uint64
        - type: string
          format: uuid
      description: Type, used for specifying point ID in user interface
      title: ExtendedPointId
    TokenizerType:
      type: string
      enum:
        - prefix
        - whitespace
        - word
        - multilingual
      title: TokenizerType
    Language:
      type: string
      enum:
        - arabic
        - azerbaijani
        - basque
        - bengali
        - catalan
        - chinese
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hebrew
        - hinglish
        - hungarian
        - indonesian
        - italian
        - japanese
        - kazakh
        - nepali
        - norwegian
        - portuguese
        - romanian
        - russian
        - slovene
        - spanish
        - swedish
        - tajik
        - turkish
      title: Language
    StopwordsSet:
      type: object
      properties:
        languages:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/Language"
          description: >-
            Set of languages to use for stopwords. Multiple pre-defined lists of
            stopwords can be combined.
        custom:
          type:
            - array
            - "null"
          items:
            type: string
          description: Custom stopwords set. Will be merged with the languages set.
      title: StopwordsSet
    StopwordsInterface:
      oneOf:
        - $ref: "#/components/schemas/Language"
        - $ref: "#/components/schemas/StopwordsSet"
      title: StopwordsInterface
    Bm25ConfigStopwords:
      oneOf:
        - $ref: "#/components/schemas/StopwordsInterface"
        - description: Any type
      description: >-
        Configuration of the stopwords filter. Supports list of pre-defined
        languages and custom stopwords. Default: initialized for specified
        `language` or English if not specified.
      title: Bm25ConfigStopwords
    Snowball:
      type: string
      enum:
        - snowball
      title: Snowball
    SnowballLanguage:
      type: string
      enum:
        - arabic
        - armenian
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hungarian
        - italian
        - norwegian
        - portuguese
        - romanian
        - russian
        - spanish
        - swedish
        - tamil
        - turkish
      description: Languages supported by snowball stemmer.
      title: SnowballLanguage
    SnowballParams:
      type: object
      properties:
        type:
          $ref: "#/components/schemas/Snowball"
        language:
          $ref: "#/components/schemas/SnowballLanguage"
      required:
        - type
        - language
      title: SnowballParams
    StemmingAlgorithm:
      oneOf:
        - $ref: "#/components/schemas/SnowballParams"
      description: Different stemming algorithms with their configs.
      title: StemmingAlgorithm
    Bm25ConfigStemmer:
      oneOf:
        - $ref: "#/components/schemas/StemmingAlgorithm"
        - description: Any type
      description: >-
        Configuration of the stemmer. Processes tokens to their root form.
        Default: initialized Snowball stemmer for specified `language` or
        English if not specified.
      title: Bm25ConfigStemmer
    Bm25Config:
      type: object
      properties:
        k:
          type: number
          format: double
          default: 1.2
          description: >-
            Controls term frequency saturation. Higher values mean term
            frequency has more impact. Default is 1.2
        b:
          type: number
          format: double
          default: 0.75
          description: >-
            Controls document length normalization. Ranges from 0 (no
            normalization) to 1 (full normalization). Higher values mean longer
            documents have less impact. Default is 0.75.
        avg_len:
          type: number
          format: double
          default: 256
          description: Expected average document length in the collection. Default is 256.
        tokenizer:
          $ref: "#/components/schemas/TokenizerType"
        language:
          type:
            - string
            - "null"
          description: >-
            Defines which language to use for text preprocessing. This parameter
            is used to construct default stopwords filter and stemmer. To
            disable language-specific processing, set this to `"language":
            "none"`. If not specified, English is assumed.
        lowercase:
          type:
            - boolean
            - "null"
          description: Lowercase the text before tokenization. Default is `true`.
        ascii_folding:
          type:
            - boolean
            - "null"
          description: >-
            If true, normalize tokens by folding accented characters to ASCII
            (e.g., "ação" -> "acao"). Default is `false`.
        stopwords:
          $ref: "#/components/schemas/Bm25ConfigStopwords"
          description: >-
            Configuration of the stopwords filter. Supports list of pre-defined
            languages and custom stopwords. Default: initialized for specified
            `language` or English if not specified.
        stemmer:
          $ref: "#/components/schemas/Bm25ConfigStemmer"
          description: >-
            Configuration of the stemmer. Processes tokens to their root form.
            Default: initialized Snowball stemmer for specified `language` or
            English if not specified.
        min_token_len:
          type:
            - integer
            - "null"
          description: >-
            Minimum token length to keep. If token is shorter than this, it will
            be discarded. Default is `None`, which means no minimum length.
        max_token_len:
          type:
            - integer
            - "null"
          description: >-
            Maximum token length to keep. If token is longer than this, it will
            be discarded. Default is `None`, which means no maximum length.
      description: Configuration of the local bm25 models.
      title: Bm25Config
    DocumentOptions:
      oneOf:
        - type: object
          additionalProperties:
            description: Any type
        - $ref: "#/components/schemas/Bm25Config"
      description: >-
        Option variants for text documents. Ether general-purpose options or
        BM25-specific options. BM25-specific will only take effect if the
        `qdrant/bm25` is specified as a model.
      title: DocumentOptions
    Document:
      type: object
      properties:
        text:
          type: string
          description: >-
            Text of the document. This field will be used as input for the
            embedding model.
        model:
          type: string
          description: >-
            Name of the model used to generate the vector. List of available
            models depends on a provider.
        options:
          $ref: "#/components/schemas/DocumentOptions"
          description: >-
            Additional options for the model, will be passed to the inference
            service as-is. See model cards for available options.
      required:
        - text
        - model
      description: >-
        WARN: Work-in-progress, unimplemented


        Text document for embedding. Requires inference infrastructure,
        unimplemented.
      title: Document
    Image:
      type: object
      properties:
        image:
          description: "Image data: base64 encoded image or an URL"
        model:
          type: string
          description: >-
            Name of the model used to generate the vector. List of available
            models depends on a provider.
        options:
          type:
            - object
            - "null"
          additionalProperties:
            description: Any type
          description: Parameters for the model Values of the parameters are model-specific
      required:
        - image
        - model
      description: >-
        WARN: Work-in-progress, unimplemented


        Image object for embedding. Requires inference infrastructure,
        unimplemented.
      title: Image
    InferenceObject:
      type: object
      properties:
        object:
          description: >-
            Arbitrary data, used as input for the embedding model. Used if the
            model requires more than one input or a custom input.
        model:
          type: string
          description: >-
            Name of the model used to generate the vector. List of available
            models depends on a provider.
        options:
          type:
            - object
            - "null"
          additionalProperties:
            description: Any type
          description: Parameters for the model Values of the parameters are model-specific
      required:
        - object
        - model
      description: >-
        WARN: Work-in-progress, unimplemented


        Custom object for embedding. Requires inference infrastructure,
        unimplemented.
      title: InferenceObject
    VectorInput:
      oneOf:
        - type: array
          items:
            type: number
            format: double
        - $ref: "#/components/schemas/SparseVector"
        - type: array
          items:
            type: array
            items:
              type: number
              format: double
        - $ref: "#/components/schemas/ExtendedPointId"
        - $ref: "#/components/schemas/Document"
        - $ref: "#/components/schemas/Image"
        - $ref: "#/components/schemas/InferenceObject"
      title: VectorInput
    Mmr:
      type: object
      properties:
        diversity:
          type:
            - number
            - "null"
          format: double
          description: >-
            Tunable parameter for the MMR algorithm. Determines the balance
            between diversity and relevance.


            A higher value favors diversity (dissimilarity to selected results),
            while a lower value favors relevance (similarity to the query
            vector).


            Must be in the range [0, 1]. Default value is 0.5.
        candidates_limit:
          type:
            - integer
            - "null"
          description: |-
            The maximum number of candidates to consider for re-ranking.

            If not specified, the `limit` value is used.
      description: Maximal Marginal Relevance (MMR) algorithm for re-ranking the points.
      title: Mmr
    NearestQueryMmr:
      oneOf:
        - $ref: "#/components/schemas/Mmr"
        - description: Any type
      description: >-
        Perform MMR (Maximal Marginal Relevance) reranking after search, using
        the same vector in this query to calculate relevance.
      title: NearestQueryMmr
    NearestQuery:
      type: object
      properties:
        nearest:
          $ref: "#/components/schemas/VectorInput"
        mmr:
          $ref: "#/components/schemas/NearestQueryMmr"
          description: >-
            Perform MMR (Maximal Marginal Relevance) reranking after search,
            using the same vector in this query to calculate relevance.
      required:
        - nearest
      title: NearestQuery
    RecommendStrategy:
      type: string
      enum:
        - average_vector
        - best_score
        - sum_scores
      description: >-
        How to use positive and negative examples to find the results, default
        is `average_vector`:


        * `average_vector` - Average positive and negative vectors and create a
        single query with the formula `query = avg_pos + avg_pos - avg_neg`.
        Then performs normal search.


        * `best_score` - Uses custom search objective. Each candidate is
        compared against all examples, its score is then chosen from the
        `max(max_pos_score, max_neg_score)`. If the `max_neg_score` is chosen
        then it is squared and negated, otherwise it is just the
        `max_pos_score`.


        * `sum_scores` - Uses custom search objective. Compares against all
        inputs, sums all the scores. Scores against positive vectors are added,
        against negatives are subtracted.
      title: RecommendStrategy
    RecommendInputStrategy:
      oneOf:
        - $ref: "#/components/schemas/RecommendStrategy"
        - description: Any type
      description: How to use the provided vectors to find the results
      title: RecommendInputStrategy
    RecommendInput:
      type: object
      properties:
        positive:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/VectorInput"
          description: Look for vectors closest to the vectors from these points
        negative:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/VectorInput"
          description: Try to avoid vectors like the vector from these points
        strategy:
          $ref: "#/components/schemas/RecommendInputStrategy"
          description: How to use the provided vectors to find the results
      title: RecommendInput
    RecommendQuery:
      type: object
      properties:
        recommend:
          $ref: "#/components/schemas/RecommendInput"
      required:
        - recommend
      title: RecommendQuery
    ContextPair:
      type: object
      properties:
        positive:
          $ref: "#/components/schemas/VectorInput"
        negative:
          $ref: "#/components/schemas/VectorInput"
      required:
        - positive
        - negative
      title: ContextPair
    DiscoverInputContext1:
      type: array
      items:
        $ref: "#/components/schemas/ContextPair"
      title: DiscoverInputContext1
    DiscoverInputContext:
      oneOf:
        - $ref: "#/components/schemas/ContextPair"
        - $ref: "#/components/schemas/DiscoverInputContext1"
        - description: Any type
      description: Search space will be constrained by these pairs of vectors
      title: DiscoverInputContext
    DiscoverInput:
      type: object
      properties:
        target:
          $ref: "#/components/schemas/VectorInput"
        context:
          $ref: "#/components/schemas/DiscoverInputContext"
          description: Search space will be constrained by these pairs of vectors
      required:
        - target
        - context
      title: DiscoverInput
    DiscoverQuery:
      type: object
      properties:
        discover:
          $ref: "#/components/schemas/DiscoverInput"
      required:
        - discover
      title: DiscoverQuery
    ContextInput1:
      type: array
      items:
        $ref: "#/components/schemas/ContextPair"
      title: ContextInput1
    ContextInput:
      oneOf:
        - $ref: "#/components/schemas/ContextPair"
        - $ref: "#/components/schemas/ContextInput1"
        - description: Any type
      title: ContextInput
    ContextQuery:
      type: object
      properties:
        context:
          $ref: "#/components/schemas/ContextInput"
      required:
        - context
      title: ContextQuery
    Direction:
      type: string
      enum:
        - asc
        - desc
      title: Direction
    OrderByDirection:
      oneOf:
        - $ref: "#/components/schemas/Direction"
        - description: Any type
      description: "Direction of ordering: `asc` or `desc`. Default is ascending."
      title: OrderByDirection
    StartFrom:
      oneOf:
        - type: integer
          format: int64
        - type: number
          format: double
        - type: string
          format: date-time
      title: StartFrom
    OrderByStartFrom:
      oneOf:
        - $ref: "#/components/schemas/StartFrom"
        - description: Any type
      description: >-
        Which payload value to start scrolling from. Default is the lowest value
        for `asc` and the highest for `desc`
      title: OrderByStartFrom
    OrderBy:
      type: object
      properties:
        key:
          type: string
          description: Payload key to order by
        direction:
          $ref: "#/components/schemas/OrderByDirection"
          description: "Direction of ordering: `asc` or `desc`. Default is ascending."
        start_from:
          $ref: "#/components/schemas/OrderByStartFrom"
          description: >-
            Which payload value to start scrolling from. Default is the lowest
            value for `asc` and the highest for `desc`
      required:
        - key
      title: OrderBy
    OrderByInterface:
      oneOf:
        - type: string
        - $ref: "#/components/schemas/OrderBy"
      title: OrderByInterface
    OrderByQuery:
      type: object
      properties:
        order_by:
          $ref: "#/components/schemas/OrderByInterface"
      required:
        - order_by
      title: OrderByQuery
    Fusion:
      type: string
      enum:
        - rrf
        - dbsf
      description: >-
        Fusion algorithm allows to combine results of multiple prefetches.


        Available fusion algorithms:


        * `rrf` - Reciprocal Rank Fusion (with default parameters) * `dbsf` -
        Distribution-Based Score Fusion
      title: Fusion
    FusionQuery:
      type: object
      properties:
        fusion:
          $ref: "#/components/schemas/Fusion"
      required:
        - fusion
      title: FusionQuery
    Rrf:
      type: object
      properties:
        k:
          type:
            - integer
            - "null"
          description: K parameter for reciprocal rank fusion
        weights:
          type:
            - array
            - "null"
          items:
            type: number
            format: double
          description: >-
            Weights for each prefetch source. Higher weight gives more influence
            on the final ranking. If not specified, all prefetches are weighted
            equally. The number of weights should match the number of
            prefetches.
      description: Parameters for Reciprocal Rank Fusion
      title: Rrf
    RrfQuery:
      type: object
      properties:
        rrf:
          $ref: "#/components/schemas/Rrf"
      required:
        - rrf
      title: RrfQuery
    ValueVariants:
      oneOf:
        - type: string
        - type: integer
          format: int64
        - type: boolean
      title: ValueVariants
    MatchValue:
      type: object
      properties:
        value:
          $ref: "#/components/schemas/ValueVariants"
      required:
        - value
      description: Exact match of the given value
      title: MatchValue
    MatchText:
      type: object
      properties:
        text:
          type: string
      required:
        - text
      description: Full-text match of the strings.
      title: MatchText
    MatchTextAny:
      type: object
      properties:
        text_any:
          type: string
      required:
        - text_any
      description: Full-text match of at least one token of the string.
      title: MatchTextAny
    MatchPhrase:
      type: object
      properties:
        phrase:
          type: string
      required:
        - phrase
      description: Full-text phrase match of the string.
      title: MatchPhrase
    AnyVariants:
      oneOf:
        - type: array
          items:
            type: string
        - type: array
          items:
            type: integer
            format: int64
      title: AnyVariants
    MatchAny:
      type: object
      properties:
        any:
          $ref: "#/components/schemas/AnyVariants"
      required:
        - any
      description: Exact match on any of the given values
      title: MatchAny
    MatchExcept:
      type: object
      properties:
        except:
          $ref: "#/components/schemas/AnyVariants"
      required:
        - except
      description: Should have at least one value not matching the any given values
      title: MatchExcept
    Match:
      oneOf:
        - $ref: "#/components/schemas/MatchValue"
        - $ref: "#/components/schemas/MatchText"
        - $ref: "#/components/schemas/MatchTextAny"
        - $ref: "#/components/schemas/MatchPhrase"
        - $ref: "#/components/schemas/MatchAny"
        - $ref: "#/components/schemas/MatchExcept"
      description: Match filter request
      title: Match
    FieldConditionMatch:
      oneOf:
        - $ref: "#/components/schemas/Match"
        - description: Any type
      description: Check if point has field with a given value
      title: FieldConditionMatch
    Range:
      type: object
      properties:
        lt:
          type:
            - number
            - "null"
          format: double
          description: point.key < range.lt
        gt:
          type:
            - number
            - "null"
          format: double
          description: point.key > range.gt
        gte:
          type:
            - number
            - "null"
          format: double
          description: point.key >= range.gte
        lte:
          type:
            - number
            - "null"
          format: double
          description: point.key <= range.lte
      description: Range filter request
      title: Range
    DatetimeRange:
      type: object
      properties:
        lt:
          type:
            - string
            - "null"
          format: date-time
          description: point.key < range.lt
        gt:
          type:
            - string
            - "null"
          format: date-time
          description: point.key > range.gt
        gte:
          type:
            - string
            - "null"
          format: date-time
          description: point.key >= range.gte
        lte:
          type:
            - string
            - "null"
          format: date-time
          description: point.key <= range.lte
      description: Range filter request
      title: DatetimeRange
    RangeInterface:
      oneOf:
        - $ref: "#/components/schemas/Range"
        - $ref: "#/components/schemas/DatetimeRange"
      title: RangeInterface
    FieldConditionRange:
      oneOf:
        - $ref: "#/components/schemas/RangeInterface"
        - description: Any type
      description: Check if points value lies in a given range
      title: FieldConditionRange
    GeoPoint:
      type: object
      properties:
        lon:
          type: number
          format: double
        lat:
          type: number
          format: double
      required:
        - lon
        - lat
      description: Geo point payload schema
      title: GeoPoint
    GeoBoundingBox:
      type: object
      properties:
        top_left:
          $ref: "#/components/schemas/GeoPoint"
        bottom_right:
          $ref: "#/components/schemas/GeoPoint"
      required:
        - top_left
        - bottom_right
      description: >-
        Geo filter request


        Matches coordinates inside the rectangle, described by coordinates of
        lop-left and bottom-right edges
      title: GeoBoundingBox
    FieldConditionGeoBoundingBox:
      oneOf:
        - $ref: "#/components/schemas/GeoBoundingBox"
        - description: Any type
      description: Check if points geolocation lies in a given area
      title: FieldConditionGeoBoundingBox
    GeoRadius:
      type: object
      properties:
        center:
          $ref: "#/components/schemas/GeoPoint"
        radius:
          type: number
          format: double
          description: Radius of the area in meters
      required:
        - center
        - radius
      description: >-
        Geo filter request


        Matches coordinates inside the circle of `radius` and center with
        coordinates `center`
      title: GeoRadius
    FieldConditionGeoRadius:
      oneOf:
        - $ref: "#/components/schemas/GeoRadius"
        - description: Any type
      description: Check if geo point is within a given radius
      title: FieldConditionGeoRadius
    GeoLineString:
      type: object
      properties:
        points:
          type: array
          items:
            $ref: "#/components/schemas/GeoPoint"
      required:
        - points
      description: Ordered sequence of GeoPoints representing the line
      title: GeoLineString
    GeoPolygon:
      type: object
      properties:
        exterior:
          $ref: "#/components/schemas/GeoLineString"
        interiors:
          type:
            - array
            - "null"
          items:
            $ref: "#/components/schemas/GeoLineString"
          description: >-
            Interior lines (if present) bound holes within the surface each
            GeoLineString must consist of a minimum of 4 points, and the first
            and last points must be the same.
      required:
        - exterior
      description: >-
        Geo filter request


        Matches coordinates inside the polygon, defined by `exterior` and
        `interiors`
      title: GeoPolygon
    FieldConditionGeoPolygon:
      oneOf:
        - $ref: "#/components/schemas/GeoPolygon"
        - description: Any type
      description: Check if geo point is within a given polygon
      title: FieldConditionGeoPolygon
    ValuesCount:
      type: object
      properties:
        lt:
          type:
            - integer
            - "null"
          description: point.key.length() < values_count.lt
        gt:
          type:
            - integer
            - "null"
          description: point.key.length() > values_count.gt
        gte:
          type:
            - integer
            - "null"
          description: point.key.length() >= values_count.gte
        lte:
          type:
            - integer
            - "null"
          description: point.key.length() <= values_count.lte
      description: Values count filter request
      title: ValuesCount
    FieldConditionValuesCount:
      oneOf:
        - $ref: "#/components/schemas/ValuesCount"
        - description: Any type
      description: Check number of values of the field
      title: FieldConditionValuesCount
    FieldCondition:
      type: object
      properties:
        key:
          type: string
          description: Payload key
        match:
          $ref: "#/components/schemas/FieldConditionMatch"
          description: Check if point has field with a given value
        range:
          $ref: "#/components/schemas/FieldConditionRange"
          description: Check if points value lies in a given range
        geo_bounding_box:
          $ref: "#/components/schemas/FieldConditionGeoBoundingBox"
          description: Check if points geolocation lies in a given area
        geo_radius:
          $ref: "#/components/schemas/FieldConditionGeoRadius"
          description: Check if geo point is within a given radius
        geo_polygon:
          $ref: "#/components/schemas/FieldConditionGeoPolygon"
          description: Check if geo point is within a given polygon
        values_count:
          $ref: "#/components/schemas/FieldConditionValuesCount"
          description: Check number of values of the field
        is_empty:
          type:
            - boolean
            - "null"
          description: >-
            Check that the field is empty, alternative syntax for `is_empty:
            "field_name"`
        is_null:
          type:
            - boolean
            - "null"
          description: >-
            Check that the field is null, alternative syntax for `is_null:
            "field_name"`
      required:
        - key
      description: All possible payload filtering conditions
      title: FieldCondition
    PayloadField:
      type: object
      properties:
        key:
          type: string
          description: Payload field name
      required:
        - key
      description: Payload field
      title: PayloadField
    IsEmptyCondition:
      type: object
      properties:
        is_empty:
          $ref: "#/components/schemas/PayloadField"
      required:
        - is_empty
      description: Select points with empty payload for a specified field
      title: IsEmptyCondition
    IsNullCondition:
      type: object
      properties:
        is_null:
          $ref: "#/components/schemas/PayloadField"
      required:
        - is_null
      description: Select points with null payload for a specified field
      title: IsNullCondition
    HasIdCondition:
      type: object
      properties:
        has_id:
          type: array
          items:
            $ref: "#/components/schemas/ExtendedPointId"
      required:
        - has_id
      description: ID-based filtering condition
      title: HasIdCondition
    HasVectorCondition:
      type: object
      properties:
        has_vector:
          type: string
      required:
        - has_vector
      description: Filter points which have specific vector assigned
      title: HasVectorCondition
    FilterShould1:
      type: array
      items:
        $ref: "#/components/schemas/Condition"
      title: FilterShould1
    FilterShould:
      oneOf:
        - $ref: "#/components/schemas/Condition"
        - $ref: "#/components/schemas/FilterShould1"
        - description: Any type
      description: At least one of those conditions should match
      title: FilterShould
    MinShould:
      type: object
      properties:
        conditions:
          type: array
          items:
            $ref: "#/components/schemas/Condition"
        min_count:
          type: integer
      required:
        - conditions
        - min_count
      title: MinShould
    FilterMinShould:
      oneOf:
        - $ref: "#/components/schemas/MinShould"
        - description: Any type
      description: At least minimum amount of given conditions should match
      title: FilterMinShould
    FilterMust1:
      type: array
      items:
        $ref: "#/components/schemas/Condition"
      title: FilterMust1
    FilterMust:
      oneOf:
        - $ref: "#/components/schemas/Condition"
        - $ref: "#/components/schemas/FilterMust1"
        - description: Any type
      description: All conditions must match
      title: FilterMust
    FilterMustNot1:
      type: array
      items:
        $ref: "#/components/schemas/Condition"
      title: FilterMustNot1
    FilterMustNot:
      oneOf:
        - $ref: "#/components/schemas/Condition"
        - $ref: "#/components/schemas/FilterMustNot1"
        - description: Any type
      description: All conditions must NOT match
      title: FilterMustNot
    Filter:
      type: object
      properties:
        should:
          $ref: "#/components/schemas/FilterShould"
          description: At least one of those conditions should match
        min_should:
          $ref: "#/components/schemas/FilterMinShould"
          description: At least minimum amount of given conditions should match
        must:
          $ref: "#/components/schemas/FilterMust"
          description: All conditions must match
        must_not:
          $ref: "#/components/schemas/FilterMustNot"
          description: All conditions must NOT match
      title: Filter
    Nested:
      type: object
      properties:
        key:
          type: string
        filter:
          $ref: "#/components/schemas/Filter"
      required:
        - key
        - filter
      description: Select points with payload for a specified nested field
      title: Nested
    NestedCondition:
      type: object
      properties:
        nested:
          $ref: "#/components/schemas/Nested"
      required:
        - nested
      title: NestedCondition
    Condition:
      oneOf:
        - $ref: "#/components/schemas/FieldCondition"
        - $ref: "#/components/schemas/IsEmptyCondition"
        - $ref: "#/components/schemas/IsNullCondition"
        - $ref: "#/components/schemas/HasIdCondition"
        - $ref: "#/components/schemas/HasVectorCondition"
        - $ref: "#/components/schemas/NestedCondition"
        - $ref: "#/components/schemas/Filter"
      title: Condition
    GeoDistanceParams:
      type: object
      properties:
        origin:
          $ref: "#/components/schemas/GeoPoint"
        to:
          type: string
          description: Payload field with the destination geo point
      required:
        - origin
        - to
      title: GeoDistanceParams
    GeoDistance:
      type: object
      properties:
        geo_distance:
          $ref: "#/components/schemas/GeoDistanceParams"
      required:
        - geo_distance
      title: GeoDistance
    DatetimeExpression:
      type: object
      properties:
        datetime:
          type: string
      required:
        - datetime
      title: DatetimeExpression
    DatetimeKeyExpression:
      type: object
      properties:
        datetime_key:
          type: string
      required:
        - datetime_key
      title: DatetimeKeyExpression
    MultExpression:
      type: object
      properties:
        mult:
          type: array
          items:
            $ref: "#/components/schemas/Expression"
      required:
        - mult
      title: MultExpression
    SumExpression:
      type: object
      properties:
        sum:
          type: array
          items:
            $ref: "#/components/schemas/Expression"
      required:
        - sum
      title: SumExpression
    NegExpression:
      type: object
      properties:
        neg:
          $ref: "#/components/schemas/Expression"
      required:
        - neg
      title: NegExpression
    AbsExpression:
      type: object
      properties:
        abs:
          $ref: "#/components/schemas/Expression"
      required:
        - abs
      title: AbsExpression
    DivParams:
      type: object
      properties:
        left:
          $ref: "#/components/schemas/Expression"
        right:
          $ref: "#/components/schemas/Expression"
        by_zero_default:
          type:
            - number
            - "null"
          format: double
      required:
        - left
        - right
      title: DivParams
    DivExpression:
      type: object
      properties:
        div:
          $ref: "#/components/schemas/DivParams"
      required:
        - div
      title: DivExpression
    SqrtExpression:
      type: object
      properties:
        sqrt:
          $ref: "#/components/schemas/Expression"
      required:
        - sqrt
      title: SqrtExpression
    PowParams:
      type: object
      properties:
        base:
          $ref: "#/components/schemas/Expression"
        exponent:
          $ref: "#/components/schemas/Expression"
      required:
        - base
        - exponent
      title: PowParams
    PowExpression:
      type: object
      properties:
        pow:
          $ref: "#/components/schemas/PowParams"
      required:
        - pow
      title: PowExpression
    ExpExpression:
      type: object
      properties:
        exp:
          $ref: "#/components/schemas/Expression"
      required:
        - exp
      title: ExpExpression
    Log10Expression:
      type: object
      properties:
        log10:
          $ref: "#/components/schemas/Expression"
      required:
        - log10
      title: Log10Expression
    LnExpression:
      type: object
      properties:
        ln:
          $ref: "#/components/schemas/Expression"
      required:
        - ln
      title: LnExpression
    DecayParamsExpressionTarget:
      oneOf:
        - $ref: "#/components/schemas/Expression"
        - description: Any type
      description: The target value to start decaying from. Defaults to 0.
      title: DecayParamsExpressionTarget
    DecayParamsExpression:
      type: object
      properties:
        x:
          $ref: "#/components/schemas/Expression"
        target:
          $ref: "#/components/schemas/DecayParamsExpressionTarget"
          description: The target value to start decaying from. Defaults to 0.
        scale:
          type:
            - number
            - "null"
          format: double
          description: >-
            The scale factor of the decay, in terms of `x`. Defaults to 1.0.
            Must be a non-zero positive number.
        midpoint:
          type:
            - number
            - "null"
          format: double
          description: >-
            The midpoint of the decay. Should be between 0 and 1.Defaults to
            0.5. Output will be this value when `|x - target| == scale`.
      required:
        - x
      title: DecayParamsExpression
    LinDecayExpression:
      type: object
      properties:
        lin_decay:
          $ref: "#/components/schemas/DecayParamsExpression"
      required:
        - lin_decay
      title: LinDecayExpression
    ExpDecayExpression:
      type: object
      properties:
        exp_decay:
          $ref: "#/components/schemas/DecayParamsExpression"
      required:
        - exp_decay
      title: ExpDecayExpression
    GaussDecayExpression:
      type: object
      properties:
        gauss_decay:
          $ref: "#/components/schemas/DecayParamsExpression"
      required:
        - gauss_decay
      title: GaussDecayExpression
    Expression:
      oneOf:
        - type: number
          format: double
        - type: string
        - $ref: "#/components/schemas/Condition"
        - $ref: "#/components/schemas/GeoDistance"
        - $ref: "#/components/schemas/DatetimeExpression"
        - $ref: "#/components/schemas/DatetimeKeyExpression"
        - $ref: "#/components/schemas/MultExpression"
        - $ref: "#/components/schemas/SumExpression"
        - $ref: "#/components/schemas/NegExpression"
        - $ref: "#/components/schemas/AbsExpression"
        - $ref: "#/components/schemas/DivExpression"
        - $ref: "#/components/schemas/SqrtExpression"
        - $ref: "#/components/schemas/PowExpression"
        - $ref: "#/components/schemas/ExpExpression"
        - $ref: "#/components/schemas/Log10Expression"
        - $ref: "#/components/schemas/LnExpression"
        - $ref: "#/components/schemas/LinDecayExpression"
        - $ref: "#/components/schemas/ExpDecayExpression"
        - $ref: "#/components/schemas/GaussDecayExpression"
      title: Expression
    FormulaQuery:
      type: object
      properties:
        formula:
          $ref: "#/components/schemas/Expression"
        defaults:
          type: object
          additionalProperties:
            description: Any type
      required:
        - formula
      title: FormulaQuery
    Sample:
      type: string
      enum:
        - random
      title: Sample
    SampleQuery:
      type: object
      properties:
        sample:
          $ref: "#/components/schemas/Sample"
      required:
        - sample
      title: SampleQuery
    FeedbackItem:
      type: object
      properties:
        example:
          $ref: "#/components/schemas/VectorInput"
        score:
          type: number
          format: double
      required:
        - example
        - score
      title: FeedbackItem
    NaiveFeedbackStrategyParams:
      type: object
      properties:
        a:
          type: number
          format: double
        b:
          type: number
          format: double
        c:
          type: number
          format: double
      required:
        - a
        - b
        - c
      title: NaiveFeedbackStrategyParams
    NaiveFeedbackStrategy:
      type: object
      properties:
        naive:
          $ref: "#/components/schemas/NaiveFeedbackStrategyParams"
      required:
        - naive
      title: NaiveFeedbackStrategy
    FeedbackStrategy:
      oneOf:
        - $ref: "#/components/schemas/NaiveFeedbackStrategy"
      title: FeedbackStrategy
    RelevanceFeedbackInput:
      type: object
      properties:
        target:
          $ref: "#/components/schemas/VectorInput"
        feedback:
          type: array
          items:
            $ref: "#/components/schemas/FeedbackItem"
        strategy:
          $ref: "#/components/schemas/FeedbackStrategy"
      required:
        - target
        - feedback
        - strategy
      title: RelevanceFeedbackInput
    RelevanceFeedbackQuery:
      type: object
      properties:
        relevance_feedback:
          $ref: "#/components/schemas/RelevanceFeedbackInput"
      required:
        - relevance_feedback
      title: RelevanceFeedbackQuery
    Query:
      oneOf:
        - $ref: "#/components/schemas/NearestQuery"
        - $ref: "#/components/schemas/RecommendQuery"
        - $ref: "#/components/schemas/DiscoverQuery"
        - $ref: "#/components/schemas/ContextQuery"
        - $ref: "#/components/schemas/OrderByQuery"
        - $ref: "#/components/schemas/FusionQuery"
        - $ref: "#/components/schemas/RrfQuery"
        - $ref: "#/components/schemas/FormulaQuery"
        - $ref: "#/components/schemas/SampleQuery"
        - $ref: "#/components/schemas/RelevanceFeedbackQuery"
      title: Query
    QueryInterface:
      oneOf:
        - $ref: "#/components/schemas/VectorInput"
        - $ref: "#/components/schemas/Query"
      title: QueryInterface
    PrefetchQuery:
      oneOf:
        - $ref: "#/components/schemas/QueryInterface"
        - description: Any type
      description: >-
        Query to perform. If missing without prefetches, returns points ordered
        by their IDs.
      title: PrefetchQuery
    PrefetchFilter:
      oneOf:
        - $ref: "#/components/schemas/Filter"
        - description: Any type
      description: >-
        Filter conditions - return only those points that satisfy the specified
        conditions.
      title: PrefetchFilter
    QuantizationSearchParams:
      type: object
      properties:
        ignore:
          type: boolean
          default: false
          description: If true, quantized vectors are ignored. Default is false.
        rescore:
          type:
            - boolean
            - "null"
          description: >-
            If true, use original vectors to re-score top-k results. Might
            require more time in case if original vectors are stored on disk. If
            not set, qdrant decides automatically apply rescoring or not.
        oversampling:
          type:
            - number
            - "null"
          format: double
          description: >-
            Oversampling factor for quantization. Default is 1.0.


            Defines how many extra vectors should be pre-selected using
            quantized index, and then re-scored using original vectors.


            For example, if `oversampling` is 2.4 and `limit` is 100, then 240
            vectors will be pre-selected using quantized index, and then top-100
            will be returned after re-scoring.
      description: Additional parameters of the search
      title: QuantizationSearchParams
    SearchParamsQuantization:
      oneOf:
        - $ref: "#/components/schemas/QuantizationSearchParams"
        - description: Any type
      description: Quantization params
      title: SearchParamsQuantization
    AcornSearchParams:
      type: object
      properties:
        enable:
          type: boolean
          default: false
          description: >-
            If true, then ACORN may be used for the HNSW search based on filters
            selectivity. Improves search recall for searches with multiple
            low-selectivity payload filters, at cost of performance.
        max_selectivity:
          type:
            - number
            - "null"
          format: double
          description: >-
            Maximum selectivity of filters to enable ACORN.


            If estimated filters selectivity is higher than this value, ACORN
            will not be used. Selectivity is estimated as: `estimated number of
            points satisfying the filters / total number of points`.


            0.0 for never, 1.0 for always. Default is 0.4.
      description: ACORN-related search parameters
      title: AcornSearchParams
    SearchParamsAcorn:
      oneOf:
        - $ref: "#/components/schemas/AcornSearchParams"
        - description: Any type
      description: ACORN search params
      title: SearchParamsAcorn
    SearchParams:
      type: object
      properties:
        hnsw_ef:
          type:
            - integer
            - "null"
          description: >-
            Params relevant to HNSW index Size of the beam in a beam-search.
            Larger the value - more accurate the result, more time required for
            search.
        exact:
          type: boolean
          default: false
          description: >-
            Search without approximation. If set to true, search may run long
            but with exact results.
        quantization:
          $ref: "#/components/schemas/SearchParamsQuantization"
          description: Quantization params
        indexed_only:
          type: boolean
          default: false
          description: >-
            If enabled, the engine will only perform search among indexed or
            small segments. Using this option prevents slow searches in case of
            delayed index, but does not guarantee that all uploaded vectors will
            be included in search results
        acorn:
          $ref: "#/components/schemas/SearchParamsAcorn"
          description: ACORN search params
      description: Additional parameters of the search
      title: SearchParams
    PrefetchParams:
      oneOf:
        - $ref: "#/components/schemas/SearchParams"
        - description: Any type
      description: Search params for when there is no prefetch
      title: PrefetchParams
    LookupLocationShardKey:
      oneOf:
        - $ref: "#/components/schemas/ShardKeySelector"
        - description: Any type
      description: >-
        Specify in which shards to look for the points, if not specified - look
        in all shards
      title: LookupLocationShardKey
    LookupLocation:
      type: object
      properties:
        collection:
          type: string
          description: Name of the collection used for lookup
        vector:
          type:
            - string
            - "null"
          description: >-
            Optional name of the vector field within the collection. If not
            provided, the default vector field will be used.
        shard_key:
          $ref: "#/components/schemas/LookupLocationShardKey"
          description: >-
            Specify in which shards to look for the points, if not specified -
            look in all shards
      required:
        - collection
      description: >-
        Defines a location to use for looking up the vector. Specifies
        collection and vector field name.
      title: LookupLocation
    PrefetchLookupFrom:
      oneOf:
        - $ref: "#/components/schemas/LookupLocation"
        - description: Any type
      description: >-
        The location to use for IDs lookup, if not specified - use the current
        collection and the 'using' vector Note: the other collection vectors
        should have the same vector size as the 'using' vector in the current
        collection
      title: PrefetchLookupFrom
    Prefetch:
      type: object
      properties:
        prefetch:
          $ref: "#/components/schemas/PrefetchPrefetch"
          description: >-
            Sub-requests to perform first. If present, the query will be
            performed on the results of the prefetches.
        query:
          $ref: "#/components/schemas/PrefetchQuery"
          description: >-
            Query to perform. If missing without prefetches, returns points
            ordered by their IDs.
        using:
          type:
            - string
            - "null"
          description: >-
            Define which vector name to use for querying. If missing, the
            default vector is used.
        filter:
          $ref: "#/components/schemas/PrefetchFilter"
          description: >-
            Filter conditions - return only those points that satisfy the
            specified conditions.
        params:
          $ref: "#/components/schemas/PrefetchParams"
          description: Search params for when there is no prefetch
        score_threshold:
          type:
            - number
            - "null"
          format: double
          description: Return points with scores better than this threshold.
        limit:
          type:
            - integer
            - "null"
          description: Max number of points to return. Default is 10.
        lookup_from:
          $ref: "#/components/schemas/PrefetchLookupFrom"
          description: >-
            The location to use for IDs lookup, if not specified - use the
            current collection and the 'using' vector Note: the other collection
            vectors should have the same vector size as the 'using' vector in
            the current collection
      title: Prefetch
    QueryRequestPrefetch1:
      type: array
      items:
        $ref: "#/components/schemas/Prefetch"
      title: QueryRequestPrefetch1
    QueryRequestPrefetch:
      oneOf:
        - $ref: "#/components/schemas/Prefetch"
        - $ref: "#/components/schemas/QueryRequestPrefetch1"
        - description: Any type
      description: >-
        Sub-requests to perform first. If present, the query will be performed
        on the results of the prefetch(es).
      title: QueryRequestPrefetch
    QueryRequestQuery:
      oneOf:
        - $ref: "#/components/schemas/QueryInterface"
        - description: Any type
      description: >-
        Query to perform. If missing without prefetches, returns points ordered
        by their IDs.
      title: QueryRequestQuery
    QueryRequestFilter:
      oneOf:
        - $ref: "#/components/schemas/Filter"
        - description: Any type
      description: >-
        Filter conditions - return only those points that satisfy the specified
        conditions.
      title: QueryRequestFilter
    QueryRequestParams:
      oneOf:
        - $ref: "#/components/schemas/SearchParams"
        - description: Any type
      description: Search params for when there is no prefetch
      title: QueryRequestParams
    WithVector:
      oneOf:
        - type: boolean
        - type: array
          items:
            type: string
      description: Options for specifying which vector to include
      title: WithVector
    QueryRequestWithVector:
      oneOf:
        - $ref: "#/components/schemas/WithVector"
        - description: Any type
      description: >-
        Options for specifying which vectors to include into the response.
        Default is false.
      title: QueryRequestWithVector
    PayloadSelectorInclude:
      type: object
      properties:
        include:
          type: array
          items:
            type: string
          description: Only include this payload keys
      required:
        - include
      title: PayloadSelectorInclude
    PayloadSelectorExclude:
      type: object
      properties:
        exclude:
          type: array
          items:
            type: string
          description: Exclude this fields from returning payload
      required:
        - exclude
      title: PayloadSelectorExclude
    PayloadSelector:
      oneOf:
        - $ref: "#/components/schemas/PayloadSelectorInclude"
        - $ref: "#/components/schemas/PayloadSelectorExclude"
      description: Specifies how to treat payload selector
      title: PayloadSelector
    WithPayloadInterface:
      oneOf:
        - type: boolean
        - type: array
          items:
            type: string
        - $ref: "#/components/schemas/PayloadSelector"
      description: Options for specifying which payload to include or not
      title: WithPayloadInterface
    QueryRequestWithPayload:
      oneOf:
        - $ref: "#/components/schemas/WithPayloadInterface"
        - description: Any type
      description: >-
        Options for specifying which payload to include or not. Default is
        false.
      title: QueryRequestWithPayload
    QueryRequestLookupFrom:
      oneOf:
        - $ref: "#/components/schemas/LookupLocation"
        - description: Any type
      description: >-
        The location to use for IDs lookup, if not specified - use the current
        collection and the 'using' vector Note: the other collection vectors
        should have the same vector size as the 'using' vector in the current
        collection
      title: QueryRequestLookupFrom
    QueryRequest:
      type: object
      properties:
        shard_key:
          $ref: "#/components/schemas/QueryRequestShardKey"
        prefetch:
          $ref: "#/components/schemas/QueryRequestPrefetch"
          description: >-
            Sub-requests to perform first. If present, the query will be
            performed on the results of the prefetch(es).
        query:
          $ref: "#/components/schemas/QueryRequestQuery"
          description: >-
            Query to perform. If missing without prefetches, returns points
            ordered by their IDs.
        using:
          type:
            - string
            - "null"
          description: >-
            Define which vector name to use for querying. If missing, the
            default vector is used.
        filter:
          $ref: "#/components/schemas/QueryRequestFilter"
          description: >-
            Filter conditions - return only those points that satisfy the
            specified conditions.
        params:
          $ref: "#/components/schemas/QueryRequestParams"
          description: Search params for when there is no prefetch
        score_threshold:
          type:
            - number
            - "null"
          format: double
          description: Return points with scores better than this threshold.
        limit:
          type:
            - integer
            - "null"
          description: Max number of points to return. Default is 10.
        offset:
          type:
            - integer
            - "null"
          description: Offset of the result. Skip this many points. Default is 0
        with_vector:
          $ref: "#/components/schemas/QueryRequestWithVector"
          description: >-
            Options for specifying which vectors to include into the response.
            Default is false.
        with_payload:
          $ref: "#/components/schemas/QueryRequestWithPayload"
          description: >-
            Options for specifying which payload to include or not. Default is
            false.
        lookup_from:
          $ref: "#/components/schemas/QueryRequestLookupFrom"
          description: >-
            The location to use for IDs lookup, if not specified - use the
            current collection and the 'using' vector Note: the other collection
            vectors should have the same vector size as the 'using' vector in
            the current collection
      title: QueryRequest
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: "#/components/schemas/HardwareUsage"
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: "#/components/schemas/ModelUsage"
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: "#/components/schemas/InferenceUsage"
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: "#/components/schemas/UsageHardware"
        inference:
          $ref: "#/components/schemas/UsageInference"
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNamePointsQueryPostResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: "#/components/schemas/Usage"
        - description: Any type
      title: >-
        CollectionsCollectionNamePointsQueryPostResponsesContentApplicationJsonSchemaUsage
    Payload:
      type: object
      additionalProperties:
        description: Any type
      title: Payload
    ScoredPointPayload:
      oneOf:
        - $ref: "#/components/schemas/Payload"
        - description: Any type
      description: Payload - values assigned to the point
      title: ScoredPointPayload
    VectorOutput:
      oneOf:
        - type: array
          items:
            type: number
            format: double
        - $ref: "#/components/schemas/SparseVector"
        - type: array
          items:
            type: array
            items:
              type: number
              format: double
      description: Vector Data stored in Point
      title: VectorOutput
    VectorStructOutput2:
      type: object
      additionalProperties:
        $ref: "#/components/schemas/VectorOutput"
      title: VectorStructOutput2
    VectorStructOutput:
      oneOf:
        - type: array
          items:
            type: number
            format: double
        - type: array
          items:
            type: array
            items:
              type: number
              format: double
        - $ref: "#/components/schemas/VectorStructOutput2"
      description: Vector data stored in Point
      title: VectorStructOutput
    ScoredPointVector:
      oneOf:
        - $ref: "#/components/schemas/VectorStructOutput"
        - description: Any type
      description: Vector of the point
      title: ScoredPointVector
    ScoredPointShardKey:
      oneOf:
        - $ref: "#/components/schemas/ShardKey"
        - description: Any type
      description: Shard Key
      title: ScoredPointShardKey
    OrderValue:
      oneOf:
        - type: integer
          format: int64
        - type: number
          format: double
      title: OrderValue
    ScoredPointOrderValue:
      oneOf:
        - $ref: "#/components/schemas/OrderValue"
        - description: Any type
      description: Order-by value
      title: ScoredPointOrderValue
    ScoredPoint:
      type: object
      properties:
        id:
          $ref: "#/components/schemas/ExtendedPointId"
        version:
          type: integer
          format: uint64
          description: Point version
        score:
          type: number
          format: double
          description: Points vector distance to the query vector
        payload:
          $ref: "#/components/schemas/ScoredPointPayload"
          description: Payload - values assigned to the point
        vector:
          $ref: "#/components/schemas/ScoredPointVector"
          description: Vector of the point
        shard_key:
          $ref: "#/components/schemas/ScoredPointShardKey"
          description: Shard Key
        order_value:
          $ref: "#/components/schemas/ScoredPointOrderValue"
          description: Order-by value
      required:
        - id
        - version
        - score
      description: Search result
      title: ScoredPoint
    QueryResponse:
      type: object
      properties:
        points:
          type: array
          items:
            $ref: "#/components/schemas/ScoredPoint"
      required:
        - points
      title: QueryResponse
    Search_query_points_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNamePointsQueryPostResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: "#/components/schemas/QueryResponse"
      title: Search_query_points_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key
```

## SDK Code Examples

```python
from qdrant_client import QdrantClient, models

client = QdrantClient(url="http://localhost:6333")

# Query nearest by ID
nearest = client.query_points(
    collection_name="{collection_name}",
    query="43cf51e2-8777-4f52-bc74-c2cbde0c8b04",
)

# Recommend on the average of these vectors
recommended = client.query_points(
    collection_name="{collection_name}",
    query=models.RecommendQuery(recommend=models.RecommendInput(
        positive=["43cf51e2-8777-4f52-bc74-c2cbde0c8b04", [0.11, 0.35, 0.6, ...]],
        negative=[[0.01, 0.45, 0.67, ...]]
    ))
)

# Fusion query
hybrid = client.query_points(
    collection_name="{collection_name}",
    prefetch=[
        models.Prefetch(
            query=models.SparseVector(indices=[1, 42], values=[0.22, 0.8]),
            using="sparse",
            limit=20,
        ),
        models.Prefetch(
            query=[0.01, 0.45, 0.67, ...],  # <-- dense vector
            using="dense",
            limit=20,
        ),
    ],
    query=models.FusionQuery(fusion=models.Fusion.RRF),
)

# 2-stage query
refined = client.query_points(
    collection_name="{collection_name}",
    prefetch=models.Prefetch(
        query=[0.01, 0.45, 0.67, ...],  # <-- dense vector
        limit=100,
    ),
    query=[
        [0.1, 0.2, ...],  # <─┐
        [0.2, 0.1, ...],  # < ├─ multi-vector
        [0.8, 0.9, ...],  # < ┘
    ],
    using="colbert",
    limit=10,
)

# Random sampling (as of 1.11.0)
sampled = client.query_points(
    collection_name="{collection_name}",
    query=models.SampleQuery(sample=models.Sample.RANDOM)
)

# Score boost depending on payload conditions (as of 1.14.0)
tag_boosted = client.query_points(
    collection_name="{collection_name}",
    prefetch=models.Prefetch(
        query=[0.2, 0.8, ...],  # <-- dense vector
        limit=50
    ),
    query=models.FormulaQuery(
        formula=models.SumExpression(sum=[
            "$score",
            models.MultExpression(mult=[0.5, models.FieldCondition(key="tag", match=models.MatchAny(any=["h1", "h2", "h3", "h4"]))]),
            models.MultExpression(mult=[0.25, models.FieldCondition(key="tag", match=models.MatchAny(any=["p", "li"]))])
        ]
    ))
)

# Score boost geographically closer points (as of 1.14.0)
geo_boosted = client.query_points(
    collection_name="{collection_name}",
    prefetch=models.Prefetch(
        query=[0.2, 0.8, ...],  # <-- dense vector
        limit=50
    ),
    query=models.FormulaQuery(
        formula=models.SumExpression(sum=[
            "$score",
            models.GaussDecayExpression(
                gauss_decay=models.DecayParamsExpression(
                    x=models.GeoDistance(
                        geo_distance=models.GeoDistanceParams(
                            origin=models.GeoPoint(
                                lat=52.504043,
                                lon=13.393236
                            ),  # Berlin
                            to="geo.location"
                        )
                    ),
                    scale=5000  # 5km
                )
            )
        ]),
        defaults={"geo.location": models.GeoPoint(lat=48.137154, lon=11.576124)}  # Munich
    )
)

```

```rust
use qdrant_client::qdrant::{
    Condition, DecayParamsExpressionBuilder, Expression, FormulaBuilder, Fusion, GeoPoint,
    PointId, PrefetchQueryBuilder, Query, QueryPointsBuilder, RecommendInputBuilder,
    Sample,
};
use qdrant_client::Qdrant;

let client = Qdrant::from_url("http://localhost:6334").build()?;

// Query nearest by ID
let _nearest = client.query(
    QueryPointsBuilder::new("{collection_name}")
        .query(PointId::from("43cf51e2-8777-4f52-bc74-c2cbde0c8b04"))
).await?;

// Recommend on the average of these vectors
let _recommendations = client.query(
    QueryPointsBuilder::new("{collection_name}")
        .query(Query::new_recommend(
            RecommendInputBuilder::default()
                .add_positive(vec![0.1; 8])
                .add_negative(PointId::from(0))
        ))
).await?;

// Fusion query
let _hybrid = client.query(
    QueryPointsBuilder::new("{collection_name}")
        .add_prefetch(PrefetchQueryBuilder::default()
            .query(vec![(1, 0.22), (42, 0.8)])
            .using("sparse")
            .limit(20u64)
        )
        .add_prefetch(PrefetchQueryBuilder::default()
            .query(vec![0.01, 0.45, 0.67])
            .using("dense")
            .limit(20u64)
        )
        .query(Fusion::Rrf)
).await?;

// 2-stage query
let _refined = client.query(
    QueryPointsBuilder::new("{collection_name}")
        .add_prefetch(PrefetchQueryBuilder::default()
            .query(vec![0.01, 0.45, 0.67])
            .limit(100u64)
        )
        .query(vec![
            vec![0.1, 0.2],
            vec![0.2, 0.1],
            vec![0.8, 0.9],
        ])
        .using("colbert")
        .limit(10u64)
).await?;

// Random sampling (as of 1.11.0)
let _sampled = client
    .query(
        QueryPointsBuilder::new("{collection_name}")
            .query(Query::new_sample(Sample::Random))
    )
    .await?;

// Score boost depending on payload conditions (as of 1.14.0)
let _tag_boosted = client.query(
    QueryPointsBuilder::new("{collection_name}")
        .add_prefetch(PrefetchQueryBuilder::default()
            .query(vec![0.01, 0.45, 0.67])
            .limit(100u64)
        )
        .query(FormulaBuilder::new(Expression::sum_with([
            Expression::score(),
            Expression::mult_with([
                Expression::constant(0.5),
                Expression::condition(Condition::matches("tag", ["h1", "h2", "h3", "h4"])),
            ]),
            Expression::mult_with([
                Expression::constant(0.25),
                Expression::condition(Condition::matches("tag", ["p", "li"])),
            ]),
        ])))
        .limit(10)
    ).await?;

// Score boost geographically closer points (as of 1.14.0)
let _geo_boosted = client.query(
    QueryPointsBuilder::new("{collection_name}")
            .add_prefetch(
                PrefetchQueryBuilder::default()
                    .query(vec![0.01, 0.45, 0.67])
                    .limit(100u64),
            )
            .query(
                FormulaBuilder::new(Expression::sum_with([
                    Expression::score(),
                    Expression::exp_decay(
                        DecayParamsExpressionBuilder::new(Expression::geo_distance_with(
                            // Berlin
                            GeoPoint { lat: 52.504043, lon: 13.393236 },
                            "geo.location",
                        ))
                        .scale(5_000.0),
                    ),
                ]))
                // Munich
                .add_default("geo.location", GeoPoint { lat: 48.137154, lon: 11.576124 }),
            )
            .limit(10),
    )
    .await?;

```

```java
import static io.qdrant.client.QueryFactory.fusion;
import static io.qdrant.client.QueryFactory.nearest;
import static io.qdrant.client.QueryFactory.recommend;
import static io.qdrant.client.VectorInputFactory.vectorInput;

import java.util.UUID;

import io.qdrant.client.QdrantClient;
import io.qdrant.client.QdrantGrpcClient;
import io.qdrant.client.grpc.Points.Fusion;
import io.qdrant.client.grpc.Points.PrefetchQuery;
import io.qdrant.client.grpc.Points.QueryPoints;
import io.qdrant.client.grpc.Points.RecommendInput;

QdrantClient client =
    new QdrantClient(QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

// Query nearest by ID
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .setQuery(nearest(UUID.fromString("43cf51e2-8777-4f52-bc74-c2cbde0c8b04")))
            .build())
    .get();

// Recommend on the average of these vectors
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .setQuery(
                recommend(
                    RecommendInput.newBuilder()
                        .addPositive(vectorInput(UUID.fromString("43cf51e2-8777-4f52-bc74-c2cbde0c8b04")))
                        .addPositive(vectorInput(0.11f, 0.35f, 0.6f))
                        .addNegative(vectorInput(0.01f, 0.45f, 0.67f))
                        .build()))
            .build())
    .get();

// Fusion query
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .addPrefetch(
                PrefetchQuery.newBuilder()
                    .setQuery(nearest(List.of(0.22f, 0.8f), List.of(1, 42)))
                    .setUsing("sparse")
                    .setLimit(20)
                    .build())
            .addPrefetch(
                PrefetchQuery.newBuilder()
                    .setQuery(nearest(List.of(0.01f, 0.45f, 0.67f)))
                    .setUsing("dense")
                    .setLimit(20)
                    .build())
            .setQuery(fusion(Fusion.RRF))
            .build())
    .get();

// 2-stage query
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .addPrefetch(
                PrefetchQuery.newBuilder()
                    .setQuery(nearest(0.01f, 0.45f, 0.67f))
                    .setLimit(100)
                    .build())
            .setQuery(
                nearest(
                    new float[][] {
                      {0.1f, 0.2f},
                      {0.2f, 0.1f},
                      {0.8f, 0.9f}
                    }))
            .setUsing("colbert")
            .setLimit(10)
            .build())
    .get();

// Random sampling (as of 1.11.0)
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .setQuery(sample(Sample.Random))
            .build())
    .get();

// Score boost depending on payload conditions (as of 1.14.0)
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .addPrefetch(
                PrefetchQuery.newBuilder()
                    .setQuery(nearest(0.01f, 0.45f, 0.67f))
                    .setLimit(100)
                    .build())
            .setQuery(
                formula(
                    Formula.newBuilder()
                        .setExpression(
                            sum(
                                SumExpression.newBuilder()
                                    .addSum(variable("$score"))
                                    .addSum(
                                        mult(
                                            MultExpression.newBuilder()
                                                .addMult(constant(0.5f))
                                                .addMult(
                                                    condition(
                                                        matchKeywords(
                                                            "tag",
                                                            List.of("h1", "h2", "h3", "h4"))))
                                                .build()))
                                    .addSum(mult(MultExpression.newBuilder()
                                    .addMult(constant(0.25f))
                                    .addMult(
                                        condition(
                                            matchKeywords(
                                                "tag",
                                                List.of("p", "li"))))
                                    .build()))
                                    .build()))
                        .build()))
            .build())
    .get();

// Score boost geographically closer points (as of 1.14.0)
client
    .queryAsync(
        QueryPoints.newBuilder()
            .setCollectionName("{collection_name}")
            .addPrefetch(
                PrefetchQuery.newBuilder()
                    .setQuery(nearest(0.01f, 0.45f, 0.67f))
                    .setLimit(100)
                    .build())
            .setQuery(
                formula(
                    Formula.newBuilder()
                        .setExpression(
                            sum(
                                SumExpression.newBuilder()
                                    .addSum(variable("$score"))
                                    .addSum(
                                        expDecay(
                                            DecayParamsExpression.newBuilder()
                                                .setX(
                                                    geoDistance(
                                                        GeoDistance.newBuilder()
                                                            .setOrigin(
                                                                GeoPoint.newBuilder()
                                                                    .setLat(52.504043)
                                                                    .setLon(13.393236)
                                                                    .build())
                                                            .setTo("geo.location")
                                                            .build()))
                                                .setScale(5000)
                                                .build()))
                                    .build()))
                        .putDefaults(
                            "geo.location",
                            value(
                                Map.of(
                                    "lat", value(48.137154),
                                    "lon", value(11.576124))))
                        .build()))
            .build())
    .get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

// Query nearest by ID
let _nearest = client.query("{collection_name", {
    query: "43cf51e2-8777-4f52-bc74-c2cbde0c8b04"
});

// Recommend on the average of these vectors
let _recommendations = client.query("{collection_name}", {
    query: {
        recommend: {
            positive: ["43cf51e2-8777-4f52-bc74-c2cbde0c8b04", [0.11, 0.35, 0.6]],
            negative: [0.01, 0.45, 0.67]
        }
    }
});

// Fusion query
let _hybrid = client.query("{collection_name}", {
    prefetch: [
        {
            query: {
                values: [0.22, 0.8],
                indices: [1, 42],
            },
            using: 'sparse',
            limit: 20,
        },
        {
            query: [0.01, 0.45, 0.67],
            using: 'dense',
            limit: 20,
        },
    ],
    query: {
        fusion: 'rrf',
    },
});

// 2-stage query
let _refined = client.query("{collection_name}", {
    prefetch: {
        query: [1, 23, 45, 67],
        limit: 100,
    },
    query: [
        [0.1, 0.2],
        [0.2, 0.1],
        [0.8, 0.9],
    ],
    using: 'colbert',
    limit: 10,
});

// Random sampling (as of 1.11.0)
let _sampled = client.query("{collection_name}", {
  query: { sample: "random" },
});

// Score boost depending on payload conditions (as of 1.14.0)
const tag_boosted = await client.query("{collection_name}", {
  prefetch: {
    query: [0.2, 0.8, 0.1, 0.9],
    limit: 50
  },
  query: {
    formula: {
      sum: [
        "$score",
        {
          mult: [ 0.5, { key: "tag", match: { any: ["h1", "h2", "h3", "h4"] }} ]
        },
        {
          mult: [ 0.25, { key: "tag", match: { any: ["p", "li"] }} ]
        }
      ]
    }
  }
});

// Score boost geographically closer points (as of 1.14.0)
const distance_boosted = await client.query("{collection_name}", {
  prefetch: {
    query: [0.2, 0.8, ...],
    limit: 50
  },
  query: {
    formula: {
      sum: [
        "$score",
        {
          gauss_decay: {
            x: {
              geo_distance: {
                origin: { lat: 52.504043, lon: 13.393236 }, // Berlin
                to: "geo.location"
              }
            },
            scale: 5000 // 5km
          }
        }
      ]
    },
    defaults: { "geo.location": { lat: 48.137154, lon: 11.576124 } } // Munich
  }
});

```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func query() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	// Query nearest by ID
	points, err := client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Query:          qdrant.NewQueryID(qdrant.NewID("43cf51e2-8777-4f52-bc74-c2cbde0c8b04")),
	})
	if err != nil {
		panic(err)
	}
	fmt.Println("Query results: ", points)

	// Recommend on the average of these vectors
	points, err = client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Query: qdrant.NewQueryRecommend(&qdrant.RecommendInput{
			Positive: []*qdrant.VectorInput{
				qdrant.NewVectorInputID(qdrant.NewID("43cf51e2-8777-4f52-bc74-c2cbde0c8b04")),
				qdrant.NewVectorInput(0.11, 0.35, 0.6),
			},
			Negative: []*qdrant.VectorInput{
				qdrant.NewVectorInput(0.01, 0.45, 0.67),
			},
		}),
	})
	if err != nil {
		panic(err)
	}
	fmt.Println("Query results: ", points)

	// Fusion query
	points, err = client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Prefetch: []*qdrant.PrefetchQuery{
			{
				Query: qdrant.NewQuerySparse([]uint32{1, 42}, []float32{0.22, 0.8}),
				Using: qdrant.PtrOf("sparse"),
			},
			{
				Query: qdrant.NewQuery(0.01, 0.45, 0.67),
				Using: qdrant.PtrOf("dense"),
			},
		},
		Query: qdrant.NewQueryFusion(qdrant.Fusion_RRF),
	})
	if err != nil {
		panic(err)
	}
	fmt.Println("Query results: ", points)

	// 2-stage query
	points, err = client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Prefetch: []*qdrant.PrefetchQuery{
			{
				Query: qdrant.NewQuery(0.01, 0.45, 0.67),
			},
		},
		Query: qdrant.NewQueryMulti([][]float32{
			{0.1, 0.2},
			{0.2, 0.1},
			{0.8, 0.9},
		}),
		Using: qdrant.PtrOf("colbert"),
	})
	if err != nil {
		panic(err)
	}
	fmt.Println("Query results: ", points)

	// Random sampling (as of 1.11.0)
	points, err = client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Query:          qdrant.NewQuerySample(qdrant.Sample_Random),
	})
	if err != nil {
		panic(err)
	}
	fmt.Println("Query results: ", points)

	// Score boost depending on payload conditions (as of 1.14.0)
	points, err = client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Prefetch: []*qdrant.PrefetchQuery{
			{
				Query: qdrant.NewQuery(0.01, 0.45, 0.67),
			},
		},
		Query: qdrant.NewQueryFormula(&qdrant.Formula{
			Expression: qdrant.NewExpressionSum(&qdrant.SumExpression{
				Sum: []*qdrant.Expression{
					qdrant.NewExpressionVariable("$score"),
					qdrant.NewExpressionMult(&qdrant.MultExpression{
						Mult: []*qdrant.Expression{
							qdrant.NewExpressionConstant(0.5),
							qdrant.NewExpressionCondition(qdrant.NewMatchKeywords("tag", "h1", "h2", "h3", "h4")),
						},
					}),
					qdrant.NewExpressionMult(&qdrant.MultExpression{
						Mult: []*qdrant.Expression{
							qdrant.NewExpressionConstant(0.25),
							qdrant.NewExpressionCondition(qdrant.NewMatchKeywords("tag", "p", "li")),
						},
					}),
				},
			}),
		}),
	})

	// Score boost geographically closer points (as of 1.14.0)
	client.Query(context.Background(), &qdrant.QueryPoints{
		CollectionName: "{collection_name}",
		Prefetch: []*qdrant.PrefetchQuery{
			{
				Query: qdrant.NewQuery(0.2, 0.8),
			},
		},
		Query: qdrant.NewQueryFormula(&qdrant.Formula{
			Expression: qdrant.NewExpressionSum(&qdrant.SumExpression{
				Sum: []*qdrant.Expression{
					qdrant.NewExpressionVariable("$score"),
					qdrant.NewExpressionExpDecay(&qdrant.DecayParamsExpression{
						X: qdrant.NewExpressionGeoDistance(&qdrant.GeoDistance{
							Origin: &qdrant.GeoPoint{
								Lat: 52.504043,
								Lon: 13.393236,
							},
							To: "geo.location",
						}),
					}),
				},
			}),
			Defaults: qdrant.NewValueMap(map[string]any{
				"geo.location": map[string]any{
					"lat": 48.137154,
					"lon": 11.576124,
				},
			}),
		}),
	})
}

```

```csharp
using Qdrant.Client;
using Qdrant.Client.Grpc;

var client = new QdrantClient("localhost", 6334);

// Query nearest by ID
await client.QueryAsync(
	collectionName: "{collection_name}",
	query: Guid.Parse("43cf51e2-8777-4f52-bc74-c2cbde0c8b04")
);

// Recommend on the average of these vectors
await client.QueryAsync(
	collectionName: "{collection_name}",
	query: new RecommendInput
	{
		Positive =
		{
			Guid.Parse("43cf51e2-8777-4f52-bc74-c2cbde0c8b04"),
			new float[] { 0.11f, 0.35f, 0.6f }
		},
		Negative = { new float[] { 0.01f, 0.45f, 0.67f } }
	}
);

// Fusion query
await client.QueryAsync(
	collectionName: "{collection_name}",
	prefetch: new List<PrefetchQuery>
	{
		new()
		{
			Query = new (float, uint)[] { (0.22f, 1), (0.8f, 42), },
			Using = "sparse",
			Limit = 20
		},
		new()
		{
			Query = new float[] { 0.01f, 0.45f, 0.67f },
			Using = "dense",
			Limit = 20
		}
	},
	query: Fusion.Rrf
);

// 2-stage query
await client.QueryAsync(
	collectionName: "{collection_name}",
	prefetch: new List<PrefetchQuery>
	{
		new() { Query = new float[] { 0.01f, 0.45f, 0.67f }, Limit = 100 }
	},
	query: new float[][] { [0.1f, 0.2f], [0.2f, 0.1f], [0.8f, 0.9f] },
	usingVector: "colbert",
	limit: 10
);

// Random sampling (as of 1.11.0)
await client.QueryAsync(
    collectionName: "{collection_name}",
    query: Sample.Random
);

// Score boost depending on payload conditions (as of 1.14.0)
await client.QueryAsync(
	collectionName: "{collection_name}",
	prefetch:
	[
		new PrefetchQuery { Query = new float[] { 0.01f, 0.45f, 0.67f }, Limit = 100 },
	],
	query: new Formula
	{
		Expression = new SumExpression
		{
			Sum =
			{
				"$score",
				new MultExpression
				{
					Mult = { 0.5f, Match("tag", ["h1", "h2", "h3", "h4"]) },
				},
				new MultExpression { Mult = { 0.25f, Match("tag", ["p", "li"]) } },
			},
		},
	},
	limit: 10
);

// Score boost geographically closer points (as of 1.14.0)
await client.QueryAsync(
	collectionName: "{collection_name}",
	prefetch:
	[
		new PrefetchQuery { Query = new float[] { 0.01f, 0.45f, 0.67f }, Limit = 100 },
	],
	query: new Formula
	{
		Expression = new SumExpression
		{
			Sum =
			{
				"$score",
				WithExpDecay(
					new()
					{
						X = new GeoDistance
						{
							Origin = new GeoPoint { Lat = 52.504043, Lon = 13.393236 },
							To = "geo.location",
						},
						Scale = 5000,
					}
				),
			},
		},
		Defaults =
		{
			["geo.location"] = new Dictionary<string, Value>
			{
				["lat"] = 48.137154,
				["lon"] = 11.576124,
			},
		},
	}
);

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/points/query")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Post.new(url)
request["api-key"] = '<apiKey>'
request["Content-Type"] = 'application/json'
request.body = "{}"

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('POST', 'http://localhost:6333/collections/collection_name/points/query', [
  'body' => '{}',
  'headers' => [
    'Content-Type' => 'application/json',
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = [
  "api-key": "<apiKey>",
  "Content-Type": "application/json"
]
let parameters = [] as [String : Any]

let postData = JSONSerialization.data(withJSONObject: parameters, options: [])

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/points/query")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "POST"
request.allHTTPHeaderFields = headers
request.httpBody = postData as Data

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```
