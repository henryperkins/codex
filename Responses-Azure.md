```JSON
{
  "/responses": {
    "post": {
      "operationId": "createResponse",
      "description": "Creates a model response.",
      "parameters": [
        {
          "name": "api-version",
          "in": "query",
          "required": false,
          "description": "The explicit Azure AI Foundry Models API version to use for this request.\n`v1` if not otherwise specified.",
          "schema": {
            "$ref": "#/components/schemas/AzureAIFoundryModelsApiVersion",
            "default": "v1"
          }
        }
      ],
      "responses": {
        "200": {
          "description": "The request has succeeded.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "id",
                  "object",
                  "created_at",
                  "error",
                  "incomplete_details",
                  "output",
                  "instructions",
                  "parallel_tool_calls",
                  "content_filters"
                ],
                "properties": {
                  "metadata": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Metadata"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "top_logprobs": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "temperature": {
                    "anyOf": [
                      {
                        "type": "number"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": 1
                  },
                  "top_p": {
                    "anyOf": [
                      {
                        "type": "number"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": 1
                  },
                  "user": {
                    "type": "string",
                    "description": "This field is being replaced by `safety_identifier` and `prompt_cache_key`. Use `prompt_cache_key` instead to maintain caching optimizations.\n  A stable identifier for your end-users.\n  Used to boost cache hit rates by better bucketing similar requests and  to help OpenAI detect and prevent abuse. [Learn more](https://platform.openai.com/docs/guides/safety-best-practices#safety-identifiers)."
                  },
                  "safety_identifier": {
                    "type": "string",
                    "description": "A stable identifier used to help detect users of your application that may be violating OpenAI's usage policies.\n  The IDs should be a string that uniquely identifies each user. We recommend hashing their username or email address, in order to avoid sending us any identifying information. [Learn more](https://platform.openai.com/docs/guides/safety-best-practices#safety-identifiers)."
                  },
                  "prompt_cache_key": {
                    "type": "string",
                    "description": "Used by OpenAI to cache responses for similar requests to optimize your cache hit rates. Replaces the `user` field. [Learn more](https://platform.openai.com/docs/guides/prompt-caching)."
                  },
                  "prompt_cache_retention": {
                    "anyOf": [
                      {
                        "type": "string",
                        "enum": [
                          "in-memory",
                          "24h"
                        ]
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "previous_response_id": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "model": {
                    "type": "string",
                    "description": "Model ID used to generate the response, like `gpt-4o` or `o3`. OpenAI\n  offers a wide range of models with different capabilities, performance\n  characteristics, and price points. Refer to the [model guide](https://platform.openai.com/docs/models)\n  to browse and compare available models."
                  },
                  "reasoning": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Reasoning"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "background": {
                    "anyOf": [
                      {
                        "type": "boolean"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "max_output_tokens": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "max_tool_calls": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "text": {
                    "$ref": "#/components/schemas/OpenAI.ResponseTextParam"
                  },
                  "tools": {
                    "$ref": "#/components/schemas/OpenAI.ToolsArray"
                  },
                  "tool_choice": {
                    "$ref": "#/components/schemas/OpenAI.ToolChoiceParam"
                  },
                  "prompt": {
                    "$ref": "#/components/schemas/OpenAI.Prompt"
                  },
                  "truncation": {
                    "anyOf": [
                      {
                        "type": "string",
                        "enum": [
                          "auto",
                          "disabled"
                        ]
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": "disabled"
                  },
                  "id": {
                    "type": "string",
                    "description": "Unique identifier for this Response."
                  },
                  "object": {
                    "type": "string",
                    "enum": [
                      "response"
                    ],
                    "description": "The object type of this resource - always set to `response`.",
                    "x-stainless-const": true
                  },
                  "status": {
                    "type": "string",
                    "enum": [
                      "completed",
                      "failed",
                      "in_progress",
                      "cancelled",
                      "queued",
                      "incomplete"
                    ],
                    "description": "The status of the response generation. One of `completed`, `failed`,\n  `in_progress`, `cancelled`, `queued`, or `incomplete`."
                  },
                  "created_at": {
                    "type": "integer",
                    "format": "unixtime",
                    "description": "Unix timestamp (in seconds) of when this Response was created."
                  },
                  "error": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.ResponseError"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "incomplete_details": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.ResponseIncompleteDetails"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "output": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/OpenAI.OutputItem"
                    },
                    "description": "An array of content items generated by the model.\n\n  - The length and order of items in the `output` array is dependent\n  on the model's response.\n  - Rather than accessing the first item in the `output` array and\n  assuming it's an `assistant` message with the content generated by\n  the model, you might consider using the `output_text` property where\n  supported in SDKs."
                  },
                  "instructions": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "array",
                        "items": {
                          "$ref": "#/components/schemas/OpenAI.InputItem"
                        }
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "output_text": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "x-stainless-skip": true
                  },
                  "usage": {
                    "$ref": "#/components/schemas/OpenAI.ResponseUsage"
                  },
                  "parallel_tool_calls": {
                    "type": "boolean",
                    "description": "Whether to allow the model to run tool calls in parallel.",
                    "default": true
                  },
                  "conversation": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Conversation"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "content_filters": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/AzureContentFilterForResponsesAPI"
                    },
                    "description": "The content filter results from RAI."
                  }
                }
              }
            },
            "text/event-stream": {
              "schema": {
                "$ref": "#/components/schemas/OpenAI.ResponseStreamEvent"
              }
            }
          }
        },
        "default": {
          "description": "An unexpected error response.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AzureErrorResponse"
              }
            }
          }
        }
      },
      "tags": [
        "Responses"
      ],
      "requestBody": {
        "required": true,
        "content": {
          "application/json": {
            "schema": {
              "$ref": "#/components/schemas/OpenAI.CreateResponse"
            }
          }
        }
      },
      "x-ms-examples": {
        "Create a response request": {
          "$ref": "./examples/responses.yaml"
        }
      }
    }
  },
  "/responses/{response_id}": {
    "get": {
      "operationId": "getResponse",
      "description": "Retrieves a model response with the given ID.",
      "parameters": [
        {
          "name": "api-version",
          "in": "query",
          "required": false,
          "description": "The explicit Azure AI Foundry Models API version to use for this request.\n`v1` if not otherwise specified.",
          "schema": {
            "$ref": "#/components/schemas/AzureAIFoundryModelsApiVersion",
            "default": "v1"
          }
        },
        {
          "name": "response_id",
          "in": "path",
          "required": true,
          "schema": {
            "type": "string"
          }
        },
        {
          "name": "include[]",
          "in": "query",
          "required": false,
          "description": "Additional fields to include in the response. See the include parameter for Response creation above for more information.",
          "schema": {
            "type": "array",
            "items": {
              "$ref": "#/components/schemas/OpenAI.IncludeEnum"
            },
            "default": []
          }
        },
        {
          "name": "stream",
          "in": "query",
          "required": false,
          "description": "If set to true, the model response data will be streamed to the client as it is generated using server-sent events.",
          "schema": {
            "type": "boolean"
          },
          "explode": false
        },
        {
          "name": "starting_after",
          "in": "query",
          "required": false,
          "description": "The sequence number of the event after which to start streaming.",
          "schema": {
            "type": "integer",
            "format": "int32"
          },
          "explode": false
        },
        {
          "name": "include_obfuscation",
          "in": "query",
          "required": false,
          "description": "When true, stream obfuscation will be enabled. Stream obfuscation adds random characters to an `obfuscation` field on streaming delta events to normalize payload sizes as a mitigation to certain side-channel attacks. These obfuscation fields are included by default, but add a small amount of overhead to the data stream. You can set `include_obfuscation` to false to optimize for bandwidth if you trust the network links between your application and the OpenAI API.",
          "schema": {
            "type": "boolean",
            "default": true
          },
          "explode": false
        }
      ],
      "responses": {
        "200": {
          "description": "The request has succeeded.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "id",
                  "object",
                  "created_at",
                  "error",
                  "incomplete_details",
                  "output",
                  "instructions",
                  "parallel_tool_calls",
                  "content_filters"
                ],
                "properties": {
                  "metadata": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Metadata"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "top_logprobs": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "temperature": {
                    "anyOf": [
                      {
                        "type": "number"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": 1
                  },
                  "top_p": {
                    "anyOf": [
                      {
                        "type": "number"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": 1
                  },
                  "user": {
                    "type": "string",
                    "description": "This field is being replaced by `safety_identifier` and `prompt_cache_key`. Use `prompt_cache_key` instead to maintain caching optimizations.\n  A stable identifier for your end-users.\n  Used to boost cache hit rates by better bucketing similar requests and  to help OpenAI detect and prevent abuse. [Learn more](https://platform.openai.com/docs/guides/safety-best-practices#safety-identifiers)."
                  },
                  "safety_identifier": {
                    "type": "string",
                    "description": "A stable identifier used to help detect users of your application that may be violating OpenAI's usage policies.\n  The IDs should be a string that uniquely identifies each user. We recommend hashing their username or email address, in order to avoid sending us any identifying information. [Learn more](https://platform.openai.com/docs/guides/safety-best-practices#safety-identifiers)."
                  },
                  "prompt_cache_key": {
                    "type": "string",
                    "description": "Used by OpenAI to cache responses for similar requests to optimize your cache hit rates. Replaces the `user` field. [Learn more](https://platform.openai.com/docs/guides/prompt-caching)."
                  },
                  "prompt_cache_retention": {
                    "anyOf": [
                      {
                        "type": "string",
                        "enum": [
                          "in-memory",
                          "24h"
                        ]
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "previous_response_id": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "model": {
                    "type": "string",
                    "description": "Model ID used to generate the response, like `gpt-4o` or `o3`. OpenAI\n  offers a wide range of models with different capabilities, performance\n  characteristics, and price points. Refer to the [model guide](https://platform.openai.com/docs/models)\n  to browse and compare available models."
                  },
                  "reasoning": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Reasoning"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "background": {
                    "anyOf": [
                      {
                        "type": "boolean"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "max_output_tokens": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "max_tool_calls": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "text": {
                    "$ref": "#/components/schemas/OpenAI.ResponseTextParam"
                  },
                  "tools": {
                    "$ref": "#/components/schemas/OpenAI.ToolsArray"
                  },
                  "tool_choice": {
                    "$ref": "#/components/schemas/OpenAI.ToolChoiceParam"
                  },
                  "prompt": {
                    "$ref": "#/components/schemas/OpenAI.Prompt"
                  },
                  "truncation": {
                    "anyOf": [
                      {
                        "type": "string",
                        "enum": [
                          "auto",
                          "disabled"
                        ]
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": "disabled"
                  },
                  "id": {
                    "type": "string",
                    "description": "Unique identifier for this Response."
                  },
                  "object": {
                    "type": "string",
                    "enum": [
                      "response"
                    ],
                    "description": "The object type of this resource - always set to `response`.",
                    "x-stainless-const": true
                  },
                  "status": {
                    "type": "string",
                    "enum": [
                      "completed",
                      "failed",
                      "in_progress",
                      "cancelled",
                      "queued",
                      "incomplete"
                    ],
                    "description": "The status of the response generation. One of `completed`, `failed`,\n  `in_progress`, `cancelled`, `queued`, or `incomplete`."
                  },
                  "created_at": {
                    "type": "integer",
                    "format": "unixtime",
                    "description": "Unix timestamp (in seconds) of when this Response was created."
                  },
                  "error": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.ResponseError"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "incomplete_details": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.ResponseIncompleteDetails"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "output": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/OpenAI.OutputItem"
                    },
                    "description": "An array of content items generated by the model.\n\n  - The length and order of items in the `output` array is dependent\n  on the model's response.\n  - Rather than accessing the first item in the `output` array and\n  assuming it's an `assistant` message with the content generated by\n  the model, you might consider using the `output_text` property where\n  supported in SDKs."
                  },
                  "instructions": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "array",
                        "items": {
                          "$ref": "#/components/schemas/OpenAI.InputItem"
                        }
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "output_text": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "x-stainless-skip": true
                  },
                  "usage": {
                    "$ref": "#/components/schemas/OpenAI.ResponseUsage"
                  },
                  "parallel_tool_calls": {
                    "type": "boolean",
                    "description": "Whether to allow the model to run tool calls in parallel.",
                    "default": true
                  },
                  "conversation": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Conversation"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "content_filters": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/AzureContentFilterForResponsesAPI"
                    },
                    "description": "The content filter results from RAI."
                  }
                }
              }
            }
          }
        },
        "default": {
          "description": "An unexpected error response.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AzureErrorResponse"
              }
            }
          }
        }
      },
      "tags": [
        "Responses"
      ]
    },
    "delete": {
      "operationId": "deleteResponse",
      "description": "Deletes a response by ID.",
      "parameters": [
        {
          "name": "api-version",
          "in": "query",
          "required": false,
          "description": "The explicit Azure AI Foundry Models API version to use for this request.\n`v1` if not otherwise specified.",
          "schema": {
            "$ref": "#/components/schemas/AzureAIFoundryModelsApiVersion",
            "default": "v1"
          }
        },
        {
          "name": "response_id",
          "in": "path",
          "required": true,
          "schema": {
            "type": "string"
          }
        }
      ],
      "responses": {
        "200": {
          "description": "The request has succeeded.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "object",
                  "id",
                  "deleted"
                ],
                "properties": {
                  "object": {
                    "type": "string",
                    "enum": [
                      "response.deleted"
                    ]
                  },
                  "id": {
                    "type": "string"
                  },
                  "deleted": {
                    "type": "boolean",
                    "enum": [
                      true
                    ]
                  }
                }
              }
            }
          }
        },
        "default": {
          "description": "An unexpected error response.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AzureErrorResponse"
              }
            }
          }
        }
      },
      "tags": [
        "Responses"
      ]
    }
  },
  "/responses/{response_id}/cancel": {
    "post": {
      "operationId": "cancelResponse",
      "description": "Cancels a model response with the given ID. Only responses created with the background parameter set to true can be cancelled.",
      "parameters": [
        {
          "name": "api-version",
          "in": "query",
          "required": false,
          "description": "The explicit Azure AI Foundry Models API version to use for this request.\n`v1` if not otherwise specified.",
          "schema": {
            "$ref": "#/components/schemas/AzureAIFoundryModelsApiVersion",
            "default": "v1"
          }
        },
        {
          "name": "response_id",
          "in": "path",
          "required": true,
          "schema": {
            "type": "string"
          }
        }
      ],
      "responses": {
        "200": {
          "description": "The request has succeeded.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "id",
                  "object",
                  "created_at",
                  "error",
                  "incomplete_details",
                  "output",
                  "instructions",
                  "parallel_tool_calls",
                  "content_filters"
                ],
                "properties": {
                  "metadata": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Metadata"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "top_logprobs": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "temperature": {
                    "anyOf": [
                      {
                        "type": "number"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": 1
                  },
                  "top_p": {
                    "anyOf": [
                      {
                        "type": "number"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": 1
                  },
                  "user": {
                    "type": "string",
                    "description": "This field is being replaced by `safety_identifier` and `prompt_cache_key`. Use `prompt_cache_key` instead to maintain caching optimizations.\n  A stable identifier for your end-users.\n  Used to boost cache hit rates by better bucketing similar requests and  to help OpenAI detect and prevent abuse. [Learn more](https://platform.openai.com/docs/guides/safety-best-practices#safety-identifiers)."
                  },
                  "safety_identifier": {
                    "type": "string",
                    "description": "A stable identifier used to help detect users of your application that may be violating OpenAI's usage policies.\n  The IDs should be a string that uniquely identifies each user. We recommend hashing their username or email address, in order to avoid sending us any identifying information. [Learn more](https://platform.openai.com/docs/guides/safety-best-practices#safety-identifiers)."
                  },
                  "prompt_cache_key": {
                    "type": "string",
                    "description": "Used by OpenAI to cache responses for similar requests to optimize your cache hit rates. Replaces the `user` field. [Learn more](https://platform.openai.com/docs/guides/prompt-caching)."
                  },
                  "prompt_cache_retention": {
                    "anyOf": [
                      {
                        "type": "string",
                        "enum": [
                          "in-memory",
                          "24h"
                        ]
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "previous_response_id": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "model": {
                    "type": "string",
                    "description": "Model ID used to generate the response, like `gpt-4o` or `o3`. OpenAI\n  offers a wide range of models with different capabilities, performance\n  characteristics, and price points. Refer to the [model guide](https://platform.openai.com/docs/models)\n  to browse and compare available models."
                  },
                  "reasoning": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Reasoning"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "background": {
                    "anyOf": [
                      {
                        "type": "boolean"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "max_output_tokens": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "max_tool_calls": {
                    "anyOf": [
                      {
                        "type": "integer"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "text": {
                    "$ref": "#/components/schemas/OpenAI.ResponseTextParam"
                  },
                  "tools": {
                    "$ref": "#/components/schemas/OpenAI.ToolsArray"
                  },
                  "tool_choice": {
                    "$ref": "#/components/schemas/OpenAI.ToolChoiceParam"
                  },
                  "prompt": {
                    "$ref": "#/components/schemas/OpenAI.Prompt"
                  },
                  "truncation": {
                    "anyOf": [
                      {
                        "type": "string",
                        "enum": [
                          "auto",
                          "disabled"
                        ]
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "default": "disabled"
                  },
                  "id": {
                    "type": "string",
                    "description": "Unique identifier for this Response."
                  },
                  "object": {
                    "type": "string",
                    "enum": [
                      "response"
                    ],
                    "description": "The object type of this resource - always set to `response`.",
                    "x-stainless-const": true
                  },
                  "status": {
                    "type": "string",
                    "enum": [
                      "completed",
                      "failed",
                      "in_progress",
                      "cancelled",
                      "queued",
                      "incomplete"
                    ],
                    "description": "The status of the response generation. One of `completed`, `failed`,\n  `in_progress`, `cancelled`, `queued`, or `incomplete`."
                  },
                  "created_at": {
                    "type": "integer",
                    "format": "unixtime",
                    "description": "Unix timestamp (in seconds) of when this Response was created."
                  },
                  "error": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.ResponseError"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "incomplete_details": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.ResponseIncompleteDetails"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "output": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/OpenAI.OutputItem"
                    },
                    "description": "An array of content items generated by the model.\n\n  - The length and order of items in the `output` array is dependent\n  on the model's response.\n  - Rather than accessing the first item in the `output` array and\n  assuming it's an `assistant` message with the content generated by\n  the model, you might consider using the `output_text` property where\n  supported in SDKs."
                  },
                  "instructions": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "array",
                        "items": {
                          "$ref": "#/components/schemas/OpenAI.InputItem"
                        }
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "output_text": {
                    "anyOf": [
                      {
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "x-stainless-skip": true
                  },
                  "usage": {
                    "$ref": "#/components/schemas/OpenAI.ResponseUsage"
                  },
                  "parallel_tool_calls": {
                    "type": "boolean",
                    "description": "Whether to allow the model to run tool calls in parallel.",
                    "default": true
                  },
                  "conversation": {
                    "anyOf": [
                      {
                        "$ref": "#/components/schemas/OpenAI.Conversation"
                      },
                      {
                        "type": "null"
                      }
                    ]
                  },
                  "content_filters": {
                    "type": "array",
                    "items": {
                      "$ref": "#/components/schemas/AzureContentFilterForResponsesAPI"
                    },
                    "description": "The content filter results from RAI."
                  }
                }
              }
            }
          }
        },
        "default": {
          "description": "An unexpected error response.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AzureErrorResponse"
              }
            }
          }
        }
      },
      "tags": [
        "Responses"
      ]
    }
  },
  "/responses/{response_id}/input_items": {
    "get": {
      "operationId": "listInputItems",
      "description": "Returns a list of input items for a given response.",
      "parameters": [
        {
          "name": "api-version",
          "in": "query",
          "required": false,
          "description": "The explicit Azure AI Foundry Models API version to use for this request.\n`v1` if not otherwise specified.",
          "schema": {
            "$ref": "#/components/schemas/AzureAIFoundryModelsApiVersion",
            "default": "v1"
          }
        },
        {
          "name": "response_id",
          "in": "path",
          "required": true,
          "schema": {
            "type": "string"
          }
        },
        {
          "name": "limit",
          "in": "query",
          "required": false,
          "description": "A limit on the number of objects to be returned. Limit can range between 1 and 100, and the\ndefault is 20.",
          "schema": {
            "type": "integer",
            "format": "int32",
            "default": 20
          },
          "explode": false
        },
        {
          "name": "order",
          "in": "query",
          "required": false,
          "description": "Sort order by the `created_at` timestamp of the objects. `asc` for ascending order and`desc`\nfor descending order.",
          "schema": {
            "type": "string",
            "enum": [
              "asc",
              "desc"
            ]
          },
          "explode": false
        },
        {
          "name": "after",
          "in": "query",
          "required": false,
          "description": "A cursor for use in pagination. `after` is an object ID that defines your place in the list.\nFor instance, if you make a list request and receive 100 objects, ending with obj_foo, your\nsubsequent call can include after=obj_foo in order to fetch the next page of the list.",
          "schema": {
            "type": "string"
          },
          "explode": false
        },
        {
          "name": "before",
          "in": "query",
          "required": false,
          "description": "A cursor for use in pagination. `before` is an object ID that defines your place in the list.\nFor instance, if you make a list request and receive 100 objects, ending with obj_foo, your\nsubsequent call can include before=obj_foo in order to fetch the previous page of the list.",
          "schema": {
            "type": "string"
          },
          "explode": false
        }
      ],
      "responses": {
        "200": {
          "description": "The request has succeeded.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/OpenAI.ResponseItemList"
              }
            }
          }
        },
        "default": {
          "description": "An unexpected error response.",
          "headers": {
            "apim-request-id": {
              "required": false,
              "description": "A request ID used for troubleshooting purposes.",
              "schema": {
                "type": "string"
              }
            }
          },
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/AzureErrorResponse"
              }
            }
          }
        }
      },
      "tags": [
        "Responses"
      ]
    }
  }
}
```