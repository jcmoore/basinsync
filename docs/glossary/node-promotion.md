[conditional-read]: <conditional-read.md>
[conditional-write]: <conditional-write.md>
[context]: <context.md>
[etag]: <etag.md>
[kernelspace]: <kernelspace.md>
[node]: <node.md>
[old-generation]: <old-generation.md>
[revision]: <revision.md>
[s3-copy]: <https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html>
[young-generation]: <young-generation.md>

# Node promotion

Moving a marked *node's*[~][node] immutable *revision*[~][revision] from the
*young*[~][young-generation] to the *old*[~][old-generation] generation.
Use *S3 COPY*[~][s3-copy] so payload bytes stay within object storage;
retry failures without relaying bytes through the server.

Promote children before their parent. Verify an existing destination by *conditional HEAD*[~][conditional-read], or
*conditionally create*[~][conditional-write] it from the young object.
Verify its length and original *ETag*[~][etag] before deleting young; record
`promoted` afterward. Copy with `x-amz-metadata-directive: REPLACE`, preserving
*context*[~][context] metadata and recording the original *ETag*[~][etag] as
`x-amz-meta-basinsync-source-etag`; *KernelSpace*[~][kernelspace] copies also keep `x-amz-meta-sha384`.
The `(promoted, marked)` index queues work with `promoted = '' AND marked > ''`;
interrupted work resumes from these durable columns.
