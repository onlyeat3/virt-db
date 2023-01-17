drop table if exists cache_config;
CREATE TABLE `cache_config` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `sql_template` varchar(500) COLLATE utf8mb4_unicode_ci NOT NULL COMMENT 'SQL模板',
  `duration` int(11) NOT NULL COMMENT '缓存有效时长',
  `cache_name` varchar(50) COLLATE utf8mb4_unicode_ci NOT NULL DEFAULT '' COMMENT '缓存名',
  `remark` varchar(200) COLLATE utf8mb4_unicode_ci NOT NULL DEFAULT '' COMMENT '备注',
  `enabled` int(11) NOT NULL DEFAULT '1' COMMENT '是否启用',
  `created_at` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
  `updated_at` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
  `created_by` bigint(20) NOT NULL DEFAULT '-1' COMMENT '创建者',
  `updated_by` bigint(20) NOT NULL DEFAULT '-1' COMMENT '最后更新者',
  PRIMARY KEY (`id`) USING BTREE,
  KEY `idx_created_at` (`created_at`) USING BTREE,
  KEY `idx_updated_at` (`updated_at`) USING BTREE,
  KEY `idx_created_by` (`created_by`) USING BTREE,
  KEY `idx_updated_by` (`updated_by`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci ROW_FORMAT=DYNAMIC COMMENT='缓存配置';

drop table if exists metric_history;
CREATE TABLE `metric_history` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `sql_str` varchar(500) COLLATE utf8mb4_unicode_ci NOT NULL COMMENT 'SQL',
  `db_server_ip` varchar(50) COLLATE utf8mb4_unicode_ci NOT NULL COMMENT '数据库ip',
  `db_server_port` varchar(50) COLLATE utf8mb4_unicode_ci NOT NULL COMMENT '数据库端口',
  `database_name` varchar(50) COLLATE utf8mb4_unicode_ci NOT NULL COMMENT '数据库',
  `avg_duration` int(11) NOT NULL COMMENT '平均耗时',
  `max_duration` int(11) NOT NULL COMMENT '最大耗时',
  `min_duration` int(11) NOT NULL COMMENT '最小耗时',
  `exec_count` int(11) NOT NULL COMMENT '执行次数',
  `cache_hit_count` int(11) NOT NULL COMMENT '缓存命中次数',
  `created_at` datetime NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
  PRIMARY KEY (`id`) USING BTREE,
  KEY `idx_created_at` (`created_at`) USING BTREE,
  KEY `idx_sql_str` (`sql_str`,`created_at`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci ROW_FORMAT=DYNAMIC COMMENT='性能指标历史';