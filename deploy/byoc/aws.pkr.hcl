packer {
  required_plugins {
    amazon = {
      version = "= 1.8.2"
      source  = "github.com/hashicorp/amazon"
    }
  }
}

variable "region" {
  type    = string
  default = "us-east-1"
}
variable "source_archive" {
  type = string
}
variable "revision" {
  type = string
}
variable "engine_profile" {
  type    = string
  default = "postgres"
  validation {
    condition     = contains(["postgres", "all"], var.engine_profile)
    error_message = "Use postgres or all."
  }
}

source "amazon-ebs" "snapshotdb" {
  region                      = var.region
  instance_type               = "m6i.xlarge"
  ssh_username                = "ubuntu"
  ssh_timeout                 = "10m"
  temporary_security_group_source_public_ip = true
  ami_name                    = "snapshotdb-${var.engine_profile}-${var.revision}-${formatdate("YYYYMMDDhhmmss", timestamp())}"
  ami_description             = "SnapshotDB BYOC appliance; initialize an empty Btrfs data volume after launch."
  source_ami_filter {
    filters = {
      name                = "ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*"
      root-device-type    = "ebs"
      virtualization-type = "hvm"
    }
    most_recent = true
    owners      = ["099720109477"]
  }
  launch_block_device_mappings {
    device_name           = "/dev/sda1"
    volume_size           = 24
    volume_type           = "gp3"
    encrypted             = true
    delete_on_termination = true
  }
  metadata_options {
    http_tokens = "required"
    http_endpoint = "enabled"
  }
  tags = {
    Project = "snapshotdb"
    Purpose = "byoc-appliance"
    Revision = var.revision
    EngineProfile = var.engine_profile
  }
}

build {
  sources = ["source.amazon-ebs.snapshotdb"]
  provisioner "file" {
    source      = var.source_archive
    destination = "/tmp/snapshotdb-source.tar.gz"
  }
  provisioner "shell" {
    inline = [
      "sudo cloud-init status --wait",
      "mkdir /tmp/snapshotdb-image-src",
      "tar -xzf /tmp/snapshotdb-source.tar.gz -C /tmp/snapshotdb-image-src",
      "sudo bash /tmp/snapshotdb-image-src/deploy/byoc/install.sh ${var.engine_profile}",
      "sudo rm -rf /tmp/snapshotdb-image-src /tmp/snapshotdb-source.tar.gz",
      "sudo cloud-init clean --logs --machine-id",
      "sudo rm -f /etc/ssh/ssh_host_* /root/.ssh/authorized_keys /home/ubuntu/.ssh/authorized_keys"
    ]
  }
  post-processor "manifest" {
    output = "byoc-manifest.json"
  }
}
